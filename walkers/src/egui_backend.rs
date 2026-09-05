//! Turning what a tile decoded into ([`crate::drawable`]) into something egui can paint.
//!
//! This is the one place which knows both, and the first thing another renderer would need
//! its own version of.

use egui::{
    Shape, Stroke,
    emath::TSTransform,
    epaint::{Vertex, WHITE_UV},
};

use crate::render::drawable::Drawable;

/// A tile's drawables, in the form egui wants them. Done once, when the tile is decoded,
/// rather than on every frame it is visible on.
pub fn to_shapes(drawables: &[Drawable]) -> Vec<Shape> {
    let mesh_of = |mesh: &crate::render::drawable::Mesh| {
        Shape::Mesh(
            egui::Mesh {
                vertices: mesh
                    .vertices
                    .iter()
                    .map(|vertex| Vertex {
                        pos: vertex.position,
                        uv: WHITE_UV,
                        color: vertex.color,
                    })
                    .collect(),
                indices: mesh.indices.to_owned(),
                ..Default::default()
            }
            .into(),
        )
    };

    drawables
        .iter()
        .flat_map(|drawable| match drawable {
            Drawable::Fill(mesh) => vec![mesh_of(mesh)],
            Drawable::Lines(run) => run
                .iter()
                .map(|line| {
                    Shape::line(line.points.to_owned(), Stroke::new(line.width, line.color))
                })
                .collect(),
        })
        .collect()
}

pub(crate) fn transformed_shape(shape: &Shape, transform: TSTransform) -> Shape {
    let mut shape = shape.to_owned();
    shape.transform(transform);

    // `line-width` is in screen pixels, so a stroke must not follow the scaling.
    keep_stroke_width(&mut shape, transform.scaling);

    shape
}

pub(crate) fn transformed_shapes(shapes: &[Shape], transform: TSTransform) -> Vec<Shape> {
    shapes
        .iter()
        .map(|shape| transformed_shape(shape, transform))
        .collect()
}

/// Undo what [`Shape::transform`] did to the stroke widths, which it scales along with
/// everything else.
fn keep_stroke_width(shape: &mut Shape, scaling: f32) {
    if scaling == 0.0 {
        return;
    }

    match shape {
        Shape::Vec(shapes) => {
            for shape in shapes {
                keep_stroke_width(shape, scaling);
            }
        }
        Shape::Path(path) => path.stroke.width /= scaling,
        Shape::LineSegment { stroke, .. } => stroke.width /= scaling,
        Shape::Circle(circle) => circle.stroke.width /= scaling,
        Shape::Ellipse(ellipse) => ellipse.stroke.width /= scaling,
        Shape::Rect(rect) => rect.stroke.width /= scaling,
        Shape::QuadraticBezier(curve) => curve.stroke.width /= scaling,
        Shape::CubicBezier(curve) => curve.stroke.width /= scaling,
        Shape::Noop | Shape::Text(_) | Shape::Mesh(_) | Shape::Callback(_) => {}
    }
}

#[cfg(test)]
mod width_tests {
    use super::*;
    use crate::expression::Context;
    use crate::render::{Geometry, render_line};
    use crate::style::{Color, Float, Paint, json};

    fn line_width_after(scaling: f32, asked: f32) -> f32 {
        let context = Context::new("LineString".to_string(), Default::default(), 10);
        let paint = Paint {
            line_color: Some(Color(json!("#000000"))),
            line_width: Some(Float(json!(asked))),
            ..Default::default()
        };
        let geometry = Geometry::LineString(geo_types::LineString::from(vec![
            (0.0f32, 0.0f32),
            (100.0, 100.0),
        ]));

        let mut drawables = Vec::new();
        render_line(&geometry, &context, &mut drawables, &paint).unwrap();

        let shapes = transformed_shapes(
            &to_shapes(&drawables),
            TSTransform {
                scaling,
                translation: Default::default(),
            },
        );

        shapes
            .iter()
            .find_map(|shape| match shape {
                Shape::LineSegment { stroke, .. } => Some(stroke.width),
                Shape::Path(path) => Some(path.stroke.width),
                _ => None,
            })
            .expect("no line")
    }

    /// A tile is drawn at whatever size the current zoom calls for, but `line-width` is in
    /// screen pixels and must not follow it.
    #[test]
    fn line_width_survives_the_transform() {
        for scaling in [256.0 / 4096.0, 362.0 / 4096.0, 511.0 / 4096.0, 1.0] {
            let width = line_width_after(scaling, 4.0);
            assert!(
                (width - 4.0).abs() < 0.001,
                "asked for 4.0, got {width} at scaling {scaling}"
            );
        }
    }

    #[test]
    fn geometry_still_scales() {
        let context = Context::new("LineString".to_string(), Default::default(), 10);
        let geometry = Geometry::LineString(geo_types::LineString::from(vec![
            (0.0f32, 0.0f32),
            (4096.0, 0.0),
        ]));

        let mut drawables = Vec::new();
        render_line(&geometry, &context, &mut drawables, &Paint::default()).unwrap();
        let shapes = transformed_shapes(
            &to_shapes(&drawables),
            TSTransform {
                scaling: 256.0 / 4096.0,
                translation: Default::default(),
            },
        );

        let points = match &shapes[0] {
            Shape::LineSegment { points, .. } => points.to_vec(),
            Shape::Path(path) => path.points.to_vec(),
            other => panic!("expected a line, got {other:?}"),
        };

        // The full extent of the tile lands on the full width it is drawn at.
        assert_eq!(points[0].x, 0.0);
        assert_eq!(points[points.len() - 1].x, 256.0);
    }
}

/// Hosting [`crate::renderer`] inside an egui app, which egui allows through a paint callback.
///
/// egui itself has no idea what backend it is being rendered with, so the app has to say, and
/// saying so is what turns this on. Without it, or under a backend which is not wgpu, tiles are
/// drawn by handing egui shapes as before.
#[cfg(feature = "wgpu")]
pub mod wgpu {
    use egui::Rect;
    use egui::emath::TSTransform;
    use egui_wgpu::wgpu;
    use egui_wgpu::{CallbackTrait, ScreenDescriptor};
    use std::sync::{Arc, Mutex};

    use crate::render::drawable::Drawable;
    use crate::render::gpu::{Renderer, key_of};

    /// The format the app renders egui to. It cannot be discovered from inside a callback.
    #[derive(Clone, Copy)]
    struct TargetFormat(wgpu::TextureFormat);

    /// Let walkers draw with its own renderer. Only makes sense when the app renders egui with
    /// wgpu, and it needs the format being rendered to:
    ///
    /// ```ignore
    /// walkers::use_wgpu(&cc.egui_ctx, cc.wgpu_render_state.as_ref().expect("wgpu").target_format);
    /// ```
    pub fn use_wgpu(ctx: &egui::Context, target_format: wgpu::TextureFormat) {
        ctx.data_mut(|data| data.insert_temp(egui::Id::NULL, TargetFormat(target_format)));
    }

    /// What [`use_wgpu`] was told, if anything.
    pub(crate) fn target_format(ctx: &egui::Context) -> Option<wgpu::TextureFormat> {
        ctx.data(|data| data.get_temp::<TargetFormat>(egui::Id::NULL))
            .map(|format| format.0)
    }

    /// One run of a tile's geometry, drawn by walkers rather than by egui.
    pub(crate) struct Run {
        /// The tile's geometry, held so that the mesh being drawn stays put.
        geometry: Arc<Vec<Drawable>>,

        /// Which of `geometry` this run is.
        index: usize,

        transform: TSTransform,
        viewport: Rect,
        format: wgpu::TextureFormat,
        frame: u64,

        /// Where this run goes, worked out by `prepare` for `paint`. One per callback rather
        /// than one per mesh, because a tile can be drawn more than once in a frame.
        placement: Mutex<Option<wgpu::BindGroup>>,
    }

    impl Run {
        pub(crate) fn callback(
            geometry: Arc<Vec<Drawable>>,
            index: usize,
            transform: TSTransform,
            viewport: Rect,
            format: wgpu::TextureFormat,
            frame: u64,
        ) -> egui::Shape {
            egui_wgpu::Callback::new_paint_callback(
                viewport,
                Self {
                    geometry,
                    index,
                    transform,
                    viewport,
                    format,
                    frame,
                    placement: Mutex::new(None),
                },
            )
            .into()
        }

        fn drawable(&self) -> Option<&Drawable> {
            self.geometry.get(self.index)
        }
    }

    impl CallbackTrait for Run {
        fn prepare(
            &self,
            device: &wgpu::Device,
            _queue: &wgpu::Queue,
            _screen_descriptor: &ScreenDescriptor,
            _egui_encoder: &mut wgpu::CommandEncoder,
            resources: &mut egui_wgpu::CallbackResources,
        ) -> Vec<wgpu::CommandBuffer> {
            let Some(drawable) = self.drawable() else {
                return Vec::new();
            };

            // Made on the first frame which draws anything, because a library has no say in
            // how the app starts up and so cannot build it earlier.
            let renderer = resources
                .entry::<Renderer>()
                .or_insert_with(|| Renderer::new(device, self.format));

            renderer.upload(device, drawable, &self.geometry, self.frame);
            renderer.forget_stale(self.frame);

            if let Ok(mut placement) = self.placement.lock() {
                *placement = Some(renderer.placement(device, self.transform, self.viewport));
            }

            Vec::new()
        }

        fn paint(
            &self,
            _info: egui::epaint::PaintCallbackInfo,
            render_pass: &mut wgpu::RenderPass<'static>,
            resources: &egui_wgpu::CallbackResources,
        ) {
            let (Some(renderer), Some(drawable)) = (resources.get::<Renderer>(), self.drawable())
            else {
                return;
            };

            let Ok(placement) = self.placement.lock() else {
                return;
            };

            if let Some(placement) = placement.as_ref() {
                renderer.draw(render_pass, key_of(drawable), placement);
            }
        }
    }
}
