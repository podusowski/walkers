//! Walkers' own renderer, which draws a tile from buffers the GPU already holds.
//!
//! Nothing in here knows about egui. It is handed a device, a queue and a render pass, and it
//! draws; whoever is hosting it decides where those come from. Inside an egui app that is
//! [`crate::egui_backend`], through a paint callback.
//!
//! The point of it is that a tile's geometry does not change as the map moves. Handing egui
//! shapes means copying and transforming every vertex on every frame; here the vertices are
//! uploaded once and the map's movement arrives as a uniform.

use egui_wgpu::wgpu;
use emath::{Rect, TSTransform};

/// What the shader needs to put a tile's vertices on the screen.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    /// Turns a tile coordinate into a point on the screen.
    scale: [f32; 2],
    offset: [f32; 2],

    /// The part of the screen being drawn to, in points, because clip space is relative to it.
    viewport_origin: [f32; 2],
    viewport_size: [f32; 2],
}

/// A vertex of a fill, as the shader reads it. Smaller than what egui uses, which carries
/// texture coordinates a filled polygon has no use for.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FillVertex {
    position: [f32; 2],
    color: [u8; 4],
}

/// A vertex of a line. A line is drawn as a quad per segment, pushed out sideways from the
/// middle of it. The push is in screen points and happens after the map's transform, which is
/// what keeps `line-width` in screen pixels without anything having to undo a scaling.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LineVertex {
    /// Where on the line this is, in tile coordinates.
    position: [f32; 2],

    /// How far to push it sideways once it is on the screen.
    extrude: [f32; 2],

    /// Which side of the line it is, -1 or 1, so that the edges can be softened.
    side: f32,

    color: [u8; 4],
}

/// How far past its real edge a line is drawn, so that the edge can be faded rather than
/// ending on a hard pixel. The same idea as egui's feathering, and the same width.
const FEATHER: f32 = 0.5;

/// Turn a run of lines into triangles. Segments are independent - no joins, no caps - which
/// shows at corners of thick lines and is the first thing to improve here.
fn line_vertices(run: &[crate::render::drawable::Line]) -> (Vec<LineVertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for line in run {
        for pair in line.points.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            let along = to - from;

            let length = along.length();
            if length == 0. {
                continue;
            }

            // Half a pixel wider than asked for, on each side, so that the fragment shader
            // has somewhere to fade out. What the line is really meant to be is worked back
            // out from this in the shader.
            let across = emath::vec2(-along.y, along.x) / length * (line.width / 2. + FEATHER);
            let corner = vertices.len() as u32;

            for (position, side) in [(from, 1.), (from, -1.), (to, 1.), (to, -1.)] {
                vertices.push(LineVertex {
                    position: [position.x, position.y],
                    extrude: [across.x * side, across.y * side],
                    side,
                    color: line.color.to_array(),
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
        }
    }

    (vertices, indices)
}

/// Frames a mesh may go undrawn before its buffers are let go. Tiles come and go as the map
/// moves, and their buffers should not outlive them by much.
const FORGET_AFTER: u64 = 120;

/// Which pipeline draws a piece of geometry.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Fill,
    Lines,
}

/// One drawable's buffers, kept between frames.
struct Uploaded {
    kind: Kind,

    /// Holds the tile's geometry, so that nothing else can be allocated at the address being
    /// used as its key while it is still in here.
    _keepalive: std::sync::Arc<Vec<crate::render::drawable::Drawable>>,

    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    indices_count: u32,
    last_drawn: u64,
}

/// Identifies a drawable by where it lives, which is unique for as long as [`Uploaded`] holds
/// onto the tile it belongs to.
pub(crate) fn key_of(drawable: &crate::render::drawable::Drawable) -> usize {
    std::ptr::from_ref(drawable) as usize
}

pub(crate) struct Renderer {
    fills: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uploaded: std::collections::HashMap<usize, Uploaded>,
}

impl Renderer {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("walkers"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("walkers"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
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

        let suffix = if format.is_srgb() {
            // The same choice egui makes, for the same reason.
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
            &format!("fs_fill_{suffix}"),
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
            &format!("fs_line_{suffix}"),
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
            ],
            std::mem::size_of::<LineVertex>() as u64,
        );

        Self {
            fills,
            lines,
            bind_group_layout,
            uploaded: Default::default(),
        }
    }

    /// Put a drawable on the GPU, unless it is already there, and say it is still wanted.
    pub(crate) fn upload(
        &mut self,
        device: &wgpu::Device,
        drawable: &crate::render::drawable::Drawable,
        keepalive: &std::sync::Arc<Vec<crate::render::drawable::Drawable>>,
        frame: u64,
    ) {
        use wgpu::util::DeviceExt as _;

        let uploaded = self.uploaded.entry(key_of(drawable)).or_insert_with(|| {
            let buffer = |contents: &[u8], usage| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("walkers"),
                    contents,
                    usage,
                })
            };

            let (kind, vertices, indices, indices_count) = match drawable {
                crate::render::drawable::Drawable::Fill(mesh) => {
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
                        buffer(bytemuck::cast_slice(&vertices), wgpu::BufferUsages::VERTEX),
                        buffer(
                            bytemuck::cast_slice(&mesh.indices),
                            wgpu::BufferUsages::INDEX,
                        ),
                        mesh.indices.len() as u32,
                    )
                }
                crate::render::drawable::Drawable::Lines(run) => {
                    let (vertices, indices) = line_vertices(run);

                    (
                        Kind::Lines,
                        buffer(bytemuck::cast_slice(&vertices), wgpu::BufferUsages::VERTEX),
                        buffer(bytemuck::cast_slice(&indices), wgpu::BufferUsages::INDEX),
                        indices.len() as u32,
                    )
                }
            };

            Uploaded {
                kind,
                _keepalive: keepalive.to_owned(),
                vertices,
                indices,
                indices_count,
                last_drawn: frame,
            }
        });

        uploaded.last_drawn = frame;
    }

    /// Let go of what has not been drawn for a while.
    pub(crate) fn forget_stale(&mut self, frame: u64) {
        self.uploaded
            .retain(|_, mesh| frame.saturating_sub(mesh.last_drawn) < FORGET_AFTER);
    }

    /// Where a tile's vertices should end up, ready to be handed to the shader.
    pub(crate) fn placement(
        &self,
        device: &wgpu::Device,
        transform: TSTransform,
        viewport: Rect,
    ) -> wgpu::BindGroup {
        use wgpu::util::DeviceExt as _;

        let uniform = Uniform {
            scale: [transform.scaling, transform.scaling],
            offset: [transform.translation.x, transform.translation.y],
            viewport_origin: [viewport.min.x, viewport.min.y],
            viewport_size: [viewport.width(), viewport.height()],
        };

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("walkers"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("walkers"),
            layout: &self.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        })
    }

    /// Draw one uploaded drawable, wherever `placement` says.
    pub(crate) fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass<'static>,
        key: usize,
        placement: &wgpu::BindGroup,
    ) {
        let Some(geometry) = self.uploaded.get(&key) else {
            return;
        };

        render_pass.set_pipeline(match geometry.kind {
            Kind::Fill => &self.fills,
            Kind::Lines => &self.lines,
        });
        render_pass.set_bind_group(0, placement, &[]);
        render_pass.set_vertex_buffer(0, geometry.vertices.slice(..));
        render_pass.set_index_buffer(geometry.indices.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..geometry.indices_count, 0, 0..1);
    }
}
