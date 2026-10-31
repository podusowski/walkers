#[cfg(feature = "vector")]
pub mod wgpu {
    use egui::Rect;
    use egui::emath::TSTransform;
    use egui_wgpu::wgpu;
    use egui_wgpu::{CallbackTrait, RenderState, ScreenDescriptor};
    use std::sync::{Arc, Mutex};

    use crate::render::drawable::Drawable;
    use crate::render::gpu::{Renderer, key_of};

    /// Call at startup to get walkers ready to draw vector tiles.
    ///
    /// ```ignore
    /// eframe::run_native("app", options, Box::new(|cc| {
    ///     walkers::install_renderer(cc.wgpu_render_state.as_ref());
    ///     Ok(Box::new(MyApp::new(cc)))
    /// }))
    /// ```
    ///
    /// # Panics
    ///
    /// If there is no render state, which means egui is not being rendered with wgpu.
    pub fn install_renderer(render_state: Option<&RenderState>) {
        let render_state = render_state.expect("walkers vector tiles require wgpu");

        render_state
            .renderer
            .write()
            .callback_resources
            .entry()
            .or_insert_with(|| Renderer::new(&render_state.device, render_state.target_format));
    }

    fn complain_about_missing_renderer_once() {
        static COMPLAINED: std::sync::Once = std::sync::Once::new();
        COMPLAINED.call_once(|| {
            log::error!(
                "There is no renderer installed. Call `walkers::install_renderer` on startup."
            );
        });
    }

    /// One run of a tile's drawables, drawn by walkers rather than by egui.
    pub(crate) struct Run {
        /// Everything the tile decoded into, held so that the one being drawn stays put.
        drawables: Arc<Vec<Drawable>>,

        /// Which of `drawables` this run is.
        index: usize,

        transform: TSTransform,

        /// Covers the whole screen rather than the run, because that is what egui sets the
        /// viewport to, and a run hanging off the edge would have its own clamped.
        viewport: Rect,

        /// What keeps the run inside its bounds, for example a tile inside its own.
        clip_rect: Rect,

        frame: u64,
        transparency: f32,

        /// Where this run goes, worked out by `prepare` for `paint`. One per callback rather
        /// than one per mesh, because a tile can be drawn more than once in a frame.
        placement: Mutex<Option<wgpu::BindGroup>>,
    }

    impl Run {
        pub(crate) fn callback(
            drawables: Arc<Vec<Drawable>>,
            index: usize,
            transform: TSTransform,
            viewport: Rect,
            clip_rect: Rect,
            frame: u64,
            transparency: f32,
        ) -> egui::Shape {
            egui_wgpu::Callback::new_paint_callback(
                viewport,
                Self {
                    drawables,
                    index,
                    transform,
                    viewport,
                    clip_rect,
                    frame,
                    transparency,
                    placement: Mutex::new(None),
                },
            )
            .into()
        }

        fn drawable(&self) -> Option<&Drawable> {
            self.drawables.get(self.index)
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

            // Put there by `install_renderer` when the app started.
            let Some(renderer) = resources.get_mut::<Renderer>() else {
                complain_about_missing_renderer_once();
                return Vec::new();
            };

            renderer.upload(device, drawable, &self.drawables, self.frame);
            renderer.forget_stale(self.frame);

            if let Ok(mut placement) = self.placement.lock() {
                *placement = Some(renderer.placement(
                    device,
                    self.transform,
                    self.viewport,
                    self.transparency,
                ));
            }

            Vec::new()
        }

        fn paint(
            &self,
            info: egui::epaint::PaintCallbackInfo,
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

            // All runs share the clip rectangle of the shape they were painted in, so each one
            // narrows down the scissor egui has set.
            let scissor = egui::epaint::ViewportInPixels::from_points(
                &info.clip_rect.intersect(self.clip_rect),
                info.pixels_per_point,
                info.screen_size_px,
            );
            if scissor.width_px <= 0 || scissor.height_px <= 0 {
                return;
            }
            render_pass.set_scissor_rect(
                scissor.left_px as u32,
                scissor.top_px as u32,
                scissor.width_px as u32,
                scissor.height_px as u32,
            );

            if let Some(placement) = placement.as_ref() {
                renderer.draw(render_pass, key_of(drawable), placement);
            }
        }
    }
}
