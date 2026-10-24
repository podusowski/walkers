#[cfg(feature = "mvt")]
use crate::{Drawable, Text};

/// What [`crate::Map::show`]'s closure gives the map to draw. It goes above the tiles, and its
/// labels are placed together with theirs, but below anything the closure paints with egui.
#[derive(Default)]
pub struct Canvas {
    /// In screen coordinates.
    #[cfg(feature = "mvt")]
    pub drawables: Vec<Drawable>,

    /// In screen coordinates.
    #[cfg(feature = "mvt")]
    pub texts: Vec<Text>,
}

#[cfg(feature = "mvt")]
pub(crate) fn shapes_of(drawables: Vec<Drawable>, ctx: &egui::Context) -> Vec<egui::Shape> {
    let drawables = std::sync::Arc::new(crate::render::merge_runs(drawables));
    let frame = ctx.cumulative_pass_nr();
    let screen = ctx.viewport_rect();

    (0..drawables.len())
        .map(|index| {
            crate::egui_backend::wgpu::Run::callback(
                drawables.to_owned(),
                index,
                egui::emath::TSTransform::IDENTITY,
                screen,
                frame,
                1.0,
            )
        })
        .collect()
}
