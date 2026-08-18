//! How the app itself looks.
//!
//! Not to be confused with [`walkers::Style`], which is about the look of the map.

use egui::{Context, FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

/// Applied once, when the app starts.
pub fn apply(ctx: &Context) {
    ctx.set_fonts(fonts());

    ctx.all_styles_mut(|style| {
        style.visuals.window_shadow.offset = [2, 3];
        style.visuals.popup_shadow.offset = [1, 2];
    });
}

/// <https://fonts.google.com/specimen/Nunito>, kept ahead of the defaults so that they still
/// serve as the fallback for whatever it has no glyph for.
fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "Nunito".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/Nunito-Regular.ttf"
        ))),
    );

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "Nunito".to_owned());

    fonts
}
