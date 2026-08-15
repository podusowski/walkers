//! How the app itself looks.
//!
//! Not to be confused with [`walkers::Style`], which is about the look of the map.

use egui::Context;

/// Applied once, when the app starts.
pub fn apply(ctx: &Context) {
    ctx.all_styles_mut(|style| {
        style.visuals.window_shadow.offset = [2, 3];
        style.visuals.popup_shadow.offset = [1, 2];
    });
}
