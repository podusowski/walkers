use std::collections::HashMap;
use std::sync::{Arc, Weak};

use egui_wgpu::wgpu;
use egui_wgpu::wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, BufferBindingType, BufferUsages, Device, RenderPipeline,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, TextureFormat,
    util::{BufferInitDescriptor, DeviceExt as _},
};
use emath::{Pos2, Rect, TSTransform, Vec2};

use crate::{Drawable, Line};

/// What the shader needs to put a tile's vertices on the screen.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    scale: [f32; 2],
    offset: [f32; 2],
    viewport_origin: [f32; 2],
    viewport_size: [f32; 2],
    transparency: f32,

    /// A uniform buffer is bound in 16 byte chunks, so the struct has to fill whole ones.
    _padding: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FillVertex {
    position: [f32; 2],
    color: [u8; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LineVertex {
    /// Where on the line this is, in tile coordinates.
    position: [f32; 2],

    /// How far to push it sideways once it is on the screen.
    extrude: [f32; 2],

    /// How far from the middle of the line it is, on the screen, positive on the left. Pushed
    /// out to a miter, a vertex is further away than this, so it is not just `extrude`'s length.
    offset: f32,

    color: [u8; 4],

    /// How far along the line this is, in tile coordinates, for the shader to find the dashes.
    along: f32,

    dasharray: [f32; 4],
}

/// How far past its real edge a line is drawn, so that the edge can be faded rather than
/// ending on a hard pixel.
const FEATHER: f32 = 0.5;

/// Dashes are worked out on the screen, so that they keep their length when a tile is
/// stretched. A pattern is passed to the shader whole, so it has to fit a fixed size.
fn dasharray_of(line: &Line) -> [f32; 4] {
    let mut pattern = line.dasharray.clone();

    // An odd pattern is repeated to make it even, like in SVG, otherwise its dashes and gaps
    // would swap on every repetition.
    if pattern.len() % 2 == 1 {
        pattern.extend_from_within(..);
    }

    if pattern.len() > 4 {
        log::warn!("Dash patterns longer than 4 are not supported, drawing a solid line.");
        return [0.; 4];
    }

    let mut packed = [0.; 4];
    packed[..pattern.len()].copy_from_slice(&pattern);
    packed
}

/// How far the point of a bend may reach, in half widths of the line, before it is cut off.
const MITER_LIMIT: f32 = 2.;

/// How the ends of two segments meeting at a bend are pushed out, on the left and on the right.
struct Join {
    before: [Vec2; 2],
    after: [Vec2; 2],

    /// The side whose point is cut off, leaving a wedge between the two segments to fill.
    cut: Option<usize>,
}

/// Where a segment going `from` one direction meets one going `to` the next, both of them
/// unit vectors. Each end is pushed out `half` the line's width.
fn join(from: Vec2, to: Vec2, half: f32, shortest: f32) -> Join {
    let (from_left, to_left) = (from.rot90() * half, to.rot90() * half);
    let separate = Join {
        before: [from_left, -from_left],
        after: [to_left, -to_left],
        cut: None,
    };

    let bisector = (from.rot90() + to.rot90()).normalized();
    let cos = bisector.dot(from.rot90());

    // Doubling back, or reaching past a whole segment, the inside of the bend would fold over
    // and the segments would cross. Overlapping there is the lesser evil.
    if cos.is_nan() || cos <= 0. || half * (1. / (cos * cos) - 1.).sqrt() > shortest {
        return separate;
    }

    let miter = bisector * half / cos;
    if 1. / cos <= MITER_LIMIT {
        return Join {
            before: [miter, -miter],
            after: [miter, -miter],
            cut: None,
        };
    }

    // The inside is the side the line turns towards, and the outside is cut off.
    if from.rot90().dot(to) > 0. {
        Join {
            before: [miter, -from_left],
            after: [miter, -to_left],
            cut: Some(1),
        }
    } else {
        Join {
            before: [from_left, -miter],
            after: [to_left, -miter],
            cut: Some(0),
        }
    }
}

/// Turn a run of lines into triangles. Segments meet along the bisector of each bend, so that
/// a translucent line is not drawn twice over there, and sharp bends are cut off. Ends are
/// square, without caps.
fn line_vertices(run: &[Line]) -> (Vec<LineVertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for line in run {
        let dasharray = dasharray_of(line);

        // Half a pixel wider than asked for, on each side, so that the fragment shader has
        // somewhere to fade out. What the line is really meant to be is worked back out from
        // this in the shader.
        let half = line.width / 2. + FEATHER;

        let mut points = line.points.clone();
        points.dedup();
        let segments: Vec<(Pos2, Pos2, f32)> = points
            .windows(2)
            .map(|pair| (pair[0], pair[1], (pair[1] - pair[0]).length()))
            .collect();
        // An outline ends where it starts, so its last segment bends into its first.
        let closed = segments.len() > 2 && points.first() == points.last();
        let bends = if closed {
            segments.len()
        } else {
            segments.len().saturating_sub(1)
        };

        // The bend after each segment, but the last one of a line which is not closed.
        let joins: Vec<Join> = (0..bends)
            .map(|index| {
                let (from, bend, before) = segments[index];
                let (_, to, after) = segments[(index + 1) % segments.len()];
                join(
                    (bend - from) / before,
                    (to - bend) / after,
                    half,
                    before.min(after),
                )
            })
            .collect();
        let first_corner = vertices.len() as u32;

        let mut travelled = 0.;
        for (index, &(from, to, length)) in segments.iter().enumerate() {
            let left = ((to - from) / length).rot90() * half;
            let square = [left, -left];
            let previous_join = match index.checked_sub(1) {
                Some(previous) => joins.get(previous),
                None if closed => joins.last(),
                None => None,
            };
            let start = previous_join.map_or(square, |join| join.after);
            let end = joins.get(index).map_or(square, |join| join.before);
            let corner = vertices.len() as u32;

            for (position, along, extrude, offset) in [
                (from, travelled, start[0], half),
                (from, travelled, start[1], -half),
                (to, travelled + length, end[0], half),
                (to, travelled + length, end[1], -half),
            ] {
                vertices.push(LineVertex {
                    position: [position.x, position.y],
                    extrude: [extrude.x, extrude.y],
                    offset,
                    color: line.color.to_array(),
                    along,
                    dasharray,
                });
            }

            indices.extend([
                corner,
                corner + 1,
                corner + 2,
                corner + 2,
                corner + 1,
                corner + 3,
            ]);

            // The wedge between the end of the previous segment and the start of this one,
            // where the outside of the bend is cut off.
            if let Some(cut) = index
                .checked_sub(1)
                .and_then(|previous| joins[previous].cut)
            {
                let previous = corner - 4;
                let inside = 1 - cut as u32;
                indices.extend([
                    previous + 2 + inside,
                    previous + 2 + cut as u32,
                    corner + cut as u32,
                ]);
            }

            travelled += length;
        }

        // The wedge where a closed line's last segment meets its first.
        if closed && let Some(cut) = joins.last().and_then(|join| join.cut) {
            let previous = vertices.len() as u32 - 4;
            let inside = 1 - cut as u32;
            indices.extend([
                previous + 2 + inside,
                previous + 2 + cut as u32,
                first_corner + cut as u32,
            ]);
        }
    }

    (vertices, indices)
}

/// Number of frames after which a mesh is freed.
const FORGET_AFTER: u64 = 120;

/// What is uploaded for which drawable, and when it is let go of. Apart from the GPU, so that it
/// can be tested without one.
struct Uploads<T> {
    /// By the address of the drawables and the index of the one among them.
    entries: HashMap<(usize, usize), Entry<T>>,

    /// Counted here rather than taken from egui, which numbers the frames of each viewport on
    /// their own.
    frame: u64,

    /// The frame of egui's which `frame` was last counted for.
    seen: Option<u64>,
}

struct Entry<T> {
    /// Keeps the address used as the key from being given to other drawables, without keeping
    /// the drawables themselves.
    owner: Weak<Vec<Drawable>>,
    last_drawn: u64,
    uploaded: T,
}

impl<T> Uploads<T> {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            frame: 0,
            seen: None,
        }
    }

    /// To be called before anything is drawn in `egui_frame`, any number of times. Lets go of
    /// what has not been drawn for a while, and right away of what nobody holds anymore, as
    /// otherwise drawables made anew every frame would pile up.
    fn begin_frame(&mut self, egui_frame: u64) {
        if self.seen.replace(egui_frame) == Some(egui_frame) {
            return;
        }

        self.frame += 1;
        let frame = self.frame;
        self.entries.retain(|_, entry| {
            entry.owner.strong_count() > 0 && frame - entry.last_drawn < FORGET_AFTER
        });
    }

    /// What is uploaded for that drawable, made with `upload` unless it is there already. Says
    /// that it is still wanted.
    fn wanted(
        &mut self,
        drawables: &Arc<Vec<Drawable>>,
        index: usize,
        upload: impl FnOnce() -> T,
    ) -> &T {
        let entry = self
            .entries
            .entry(key_of(drawables, index))
            .or_insert_with(|| Entry {
                owner: Arc::downgrade(drawables),
                last_drawn: self.frame,
                uploaded: upload(),
            });
        entry.last_drawn = self.frame;
        &entry.uploaded
    }

    fn get(&self, drawables: &Arc<Vec<Drawable>>, index: usize) -> Option<&T> {
        self.entries
            .get(&key_of(drawables, index))
            .map(|entry| &entry.uploaded)
    }
}

fn key_of(drawables: &Arc<Vec<Drawable>>, index: usize) -> (usize, usize) {
    (Arc::as_ptr(drawables) as usize, index)
}

/// Which pipeline draws a piece of geometry.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Fill,
    Lines,
}

/// One drawable's buffers, kept between frames.
struct Uploaded {
    kind: Kind,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    indices_count: u32,
}

pub(crate) struct Renderer {
    fills: RenderPipeline,
    lines: RenderPipeline,
    bind_group_layout: BindGroupLayout,
    uploads: Uploads<Uploaded>,
}

impl Renderer {
    pub(crate) fn new(device: &Device, format: TextureFormat) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("walkers"),
            source: ShaderSource::Wgsl(include_str!("gpu.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("walkers"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("walkers"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let fragment_suffix = if format.is_srgb() {
            "linear_framebuffer"
        } else {
            "gamma_framebuffer"
        };

        let pipeline = |vertex_entry: &str,
                        fragment_entry: &str,
                        attributes: &[wgpu::VertexAttribute],
                        stride: u64| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("walkers"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vertex_entry),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: stride,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes,
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fragment_entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        // egui works in premultiplied alpha, and so must anything drawn beside it.
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::One,
                                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                operation: wgpu::BlendOperation::Add,
                            },
                            alpha: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::OneMinusDstAlpha,
                                dst_factor: wgpu::BlendFactor::One,
                                operation: wgpu::BlendOperation::Add,
                            },
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };

        let fills = pipeline(
            "vs_fill",
            &format!("fs_fill_{fragment_suffix}"),
            &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Unorm8x4,
                    offset: 8,
                    shader_location: 1,
                },
            ],
            std::mem::size_of::<FillVertex>() as u64,
        );

        let lines = pipeline(
            "vs_line",
            &format!("fs_line_{fragment_suffix}"),
            &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 8,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 16,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Unorm8x4,
                    offset: 20,
                    shader_location: 3,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 24,
                    shader_location: 4,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 28,
                    shader_location: 5,
                },
            ],
            std::mem::size_of::<LineVertex>() as u64,
        );

        Self {
            fills,
            lines,
            bind_group_layout,
            uploads: Uploads::new(),
        }
    }

    /// Put the drawable at `index` of `drawables` on the GPU, unless it is already there, and say
    /// it is still wanted. `frame` is egui's.
    pub(crate) fn upload(
        &mut self,
        device: &Device,
        drawables: &Arc<Vec<Drawable>>,
        index: usize,
        frame: u64,
    ) {
        let Some(drawable) = drawables.get(index) else {
            return;
        };

        self.uploads.begin_frame(frame);
        self.uploads.wanted(drawables, index, || {
            let buffer = |contents: &[u8], usage| {
                device.create_buffer_init(&BufferInitDescriptor {
                    label: Some("walkers"),
                    contents,
                    usage,
                })
            };

            let (kind, vertices, indices, indices_count) = match drawable {
                Drawable::Fill(mesh) => {
                    let vertices: Vec<FillVertex> = mesh
                        .vertices
                        .iter()
                        .map(|vertex| FillVertex {
                            position: [vertex.position.x, vertex.position.y],
                            color: vertex.color.to_array(),
                        })
                        .collect();

                    (
                        Kind::Fill,
                        buffer(bytemuck::cast_slice(&vertices), BufferUsages::VERTEX),
                        buffer(bytemuck::cast_slice(&mesh.indices), BufferUsages::INDEX),
                        mesh.indices.len() as u32,
                    )
                }
                Drawable::Lines(run) => {
                    let (vertices, indices) = line_vertices(run);

                    (
                        Kind::Lines,
                        buffer(bytemuck::cast_slice(&vertices), BufferUsages::VERTEX),
                        buffer(bytemuck::cast_slice(&indices), BufferUsages::INDEX),
                        indices.len() as u32,
                    )
                }
            };

            Uploaded {
                kind,
                vertices,
                indices,
                indices_count,
            }
        });
    }

    /// Where a tile's vertices should end up, ready to be handed to the shader.
    pub(crate) fn placement(
        &self,
        device: &Device,
        transform: TSTransform,
        viewport: Rect,
        transparency: f32,
    ) -> BindGroup {
        let uniform = Uniform {
            scale: [transform.scaling, transform.scaling],
            offset: [transform.translation.x, transform.translation.y],
            viewport_origin: [viewport.min.x, viewport.min.y],
            viewport_size: [viewport.width(), viewport.height()],
            transparency,
            _padding: Default::default(),
        };

        let buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("walkers"),
            contents: bytemuck::bytes_of(&uniform),
            usage: BufferUsages::UNIFORM,
        });

        device.create_bind_group(&BindGroupDescriptor {
            label: Some("walkers"),
            layout: &self.bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        })
    }

    /// Draw one uploaded drawable, wherever `placement` says.
    pub(crate) fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass<'static>,
        drawables: &Arc<Vec<Drawable>>,
        index: usize,
        placement: &BindGroup,
    ) {
        let Some(uploaded) = self.uploads.get(drawables, index) else {
            return;
        };

        render_pass.set_pipeline(match uploaded.kind {
            Kind::Fill => &self.fills,
            Kind::Lines => &self.lines,
        });
        render_pass.set_bind_group(0, placement, &[]);
        render_pass.set_vertex_buffer(0, uploaded.vertices.slice(..));
        render_pass.set_index_buffer(uploaded.indices.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..uploaded.indices_count, 0, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::drawable::Line;
    use ecolor::Color32;
    use emath::pos2;

    fn drawables() -> Arc<Vec<Drawable>> {
        Arc::new(Vec::new())
    }

    /// Like those of a layer which makes them anew every frame. Nothing can draw them again.
    #[test]
    fn drawables_nobody_holds_anymore_are_forgotten_in_the_next_frame() {
        let mut uploads = Uploads::new();
        let made_this_frame = drawables();

        uploads.begin_frame(0);
        uploads.wanted(&made_this_frame, 0, || ());
        uploads.wanted(&made_this_frame, 1, || ());
        assert_eq!(uploads.entries.len(), 2);

        drop(made_this_frame);
        uploads.begin_frame(1);
        assert!(uploads.entries.is_empty());
    }

    /// Like those of a tile, which are uploaded once for as long as it is drawn.
    #[test]
    fn drawables_which_keep_being_drawn_stay() {
        let mut uploads = Uploads::new();
        let tile = drawables();
        let mut uploaded = 0;

        for frame in 0..FORGET_AFTER * 2 {
            uploads.begin_frame(frame);
            uploads.wanted(&tile, 0, || uploaded += 1);
        }

        assert_eq!(uploaded, 1);
    }

    /// Like those of a tile which is still cached, but no longer visible.
    #[test]
    fn drawables_not_drawn_for_a_while_are_forgotten() {
        let mut uploads = Uploads::new();
        let tile = drawables();

        uploads.begin_frame(0);
        uploads.wanted(&tile, 0, || ());

        for frame in 1..FORGET_AFTER {
            uploads.begin_frame(frame);
        }
        assert!(uploads.get(&tile, 0).is_some());

        uploads.begin_frame(FORGET_AFTER);
        assert!(uploads.get(&tile, 0).is_none());
    }

    /// Every drawable drawn in a frame begins it, and viewports number their frames on their
    /// own, so the same number says nothing about how much time has passed.
    #[test]
    fn frame_begun_again_is_still_the_same_frame() {
        let mut uploads = Uploads::new();
        let tile = drawables();

        uploads.begin_frame(0);
        uploads.wanted(&tile, 0, || ());

        for _ in 0..FORGET_AFTER * 2 {
            uploads.begin_frame(1);
        }
        assert!(uploads.get(&tile, 0).is_some());
    }

    fn horizontal(width: f32) -> Vec<Line> {
        vec![Line {
            points: vec![pos2(0., 0.), pos2(10., 0.)],
            width,
            color: Color32::WHITE,
            dasharray: Vec::new(),
        }]
    }

    /// A tile is drawn at whatever size the current zoom calls for, but `line-width` is in
    /// screen pixels. Pushing the vertices out after the transform is what keeps it that way,
    /// so how far they are pushed is what has to be right.
    #[test]
    fn a_line_is_pushed_out_by_half_its_width() {
        let (vertices, _) = line_vertices(&horizontal(4.));

        for vertex in &vertices {
            let [x, y] = vertex.extrude;
            assert!(
                x.abs() < f32::EPSILON,
                "pushed along the line, not across it"
            );
            assert!((y.abs() - (2. + FEATHER)).abs() < 0.001, "pushed {y} out");
        }
    }

    /// Every segment is two triangles, and the ends are not joined up to anything.
    #[test]
    fn every_segment_is_a_quad() {
        let (vertices, indices) = line_vertices(&horizontal(1.));

        assert_eq!(vertices.len(), 4);
        assert_eq!(indices.len(), 6);
    }

    /// How many of the line's triangles cover `point`, with the map drawn at its own size.
    fn layers_at(vertices: &[LineVertex], indices: &[u32], point: emath::Pos2) -> usize {
        let on_screen = |index: u32| {
            let vertex = &vertices[index as usize];
            emath::pos2(
                vertex.position[0] + vertex.extrude[0],
                vertex.position[1] + vertex.extrude[1],
            )
        };
        let cross = |a: emath::Pos2, b: emath::Pos2, c: emath::Pos2| {
            (b - a).x * (c - a).y - (b - a).y * (c - a).x
        };

        indices
            .chunks(3)
            .filter(|triangle| {
                let [a, b, c] = [0, 1, 2].map(|corner| on_screen(triangle[corner]));
                let sides = [cross(a, b, point), cross(b, c, point), cross(c, a, point)];
                sides.iter().all(|side| *side > 0.) || sides.iter().all(|side| *side < 0.)
            })
            .count()
    }

    /// A translucent line drawn twice over at its bends shows knots there, and one with a
    /// notch cut out of the outside of each bend looks frayed.
    #[test]
    fn a_bend_is_covered_once_without_gaps() {
        // Right angles, and a sharp turn which is cut off at the outside.
        for points in [
            vec![pos2(0., 0.), pos2(20., 0.), pos2(20., 20.)],
            vec![pos2(0., 0.), pos2(20., 0.), pos2(20., -20.)],
            vec![pos2(0., 0.), pos2(20., 0.), pos2(2., 6.)],
        ] {
            let (vertices, indices) = line_vertices(&[Line {
                points: points.clone(),
                width: 4.,
                color: Color32::WHITE,
                dasharray: Vec::new(),
            }]);

            // A grid offset from whole numbers, so that no point lies on an edge.
            for x in -10..=30 {
                for y in -30..=30 {
                    let point = pos2(x as f32 + 0.13, y as f32 + 0.37);
                    assert!(
                        layers_at(&vertices, &indices, point) <= 1,
                        "{points:?}: {point:?} covered more than once"
                    );
                }
            }

            // Just past the bend, outwards, which is covered even where its point is cut off.
            let [before, bend, after] = [points[0], points[1], points[2]];
            let outward =
                -((before - bend).normalized() + (after - bend).normalized()).normalized();
            let point = bend + outward * 0.3;
            assert_eq!(
                layers_at(&vertices, &indices, point),
                1,
                "{points:?}: {point:?} at the outside of the bend"
            );
        }
    }

    /// An outline ends where it starts, and that corner is a bend like any other.
    #[test]
    fn a_closed_line_is_joined_where_it_starts() {
        let (vertices, indices) = line_vertices(&[Line {
            points: vec![
                pos2(0., 0.),
                pos2(20., 0.),
                pos2(20., 20.),
                pos2(0., 20.),
                pos2(0., 0.),
            ],
            width: 4.,
            color: Color32::WHITE,
            dasharray: Vec::new(),
        }]);

        for x in -10..=30 {
            for y in -10..=30 {
                let point = pos2(x as f32 + 0.13, y as f32 + 0.37);
                assert!(
                    layers_at(&vertices, &indices, point) <= 1,
                    "{point:?} covered more than once"
                );
            }
        }

        // Just outside the corner the line starts and ends at, on either side of where its
        // first and last segments meet.
        for point in [pos2(-1.3, -0.4), pos2(-0.4, -1.3)] {
            assert_eq!(layers_at(&vertices, &indices, point), 1, "{point:?}");
        }
    }

    /// A line which doubles back on itself has nothing to be pushed out from.
    #[test]
    fn a_segment_going_nowhere_is_skipped() {
        let (vertices, _) = line_vertices(&[Line {
            points: vec![pos2(3., 3.), pos2(3., 3.)],
            width: 2.,
            color: Color32::WHITE,
            dasharray: Vec::new(),
        }]);

        assert!(vertices.is_empty());
    }
}
