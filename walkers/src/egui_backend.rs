//! Where walkers meets egui.
//!
//! Tiles are drawn by [`crate::render::gpu`], which this hosts through a paint callback.
//! [`to_shapes`] is for geometry which is worked out afresh every frame, such as a GeoJSON
//! overlay following the map - there is nothing for the GPU to hold on to there, so handing
//! egui shapes is the right thing to do.

use egui::{
    Shape, Stroke,
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
/// Hosting [`crate::render::gpu`] inside an egui app, which egui allows through a paint callback.
///
/// egui itself has no idea what backend it is being rendered with, so the app has to say, and
/// saying so is what turns this on. Without it, or under a backend which is not wgpu, tiles are
/// drawn by handing egui shapes as before.
#[cfg(feature = "mvt")]
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

    /// What [`use_wgpu`] was told. There is nothing to fall back on if it was not told
    /// anything, so rather than leave the map empty in silence, say so once.
    pub(crate) fn target_format(ctx: &egui::Context) -> Option<wgpu::TextureFormat> {
        let format = ctx
            .data(|data| data.get_temp::<TargetFormat>(egui::Id::NULL))
            .map(|format| format.0);

        if format.is_none() {
            static COMPLAINED: std::sync::Once = std::sync::Once::new();
            COMPLAINED.call_once(|| {
                log::error!(
                    "Vector tiles are drawn with wgpu, and nothing has said what is being \
                     rendered to. Call `walkers::use_wgpu` when the app starts, or the map \
                     will stay empty."
                );
            });
        }

        format
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
