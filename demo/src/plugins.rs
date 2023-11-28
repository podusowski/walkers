use egui::{Color32, Painter, Response};
use walkers::{
    extras::{Image, Images, Place, Places, Style, Texture},
    Plugin, Projector,
};

use crate::places;

/// Creates a built-in `Places` plugin with some predefined places.
pub fn places() -> impl Plugin {
    Places::new(vec![
        Place {
            position: places::wroclaw_glowny(),
            label: "Wrocław Główny\ntrain station".to_owned(),
            symbol: '🚆',
            style: Style::default(),
        },
        Place {
            position: places::dworcowa_bus_stop(),
            label: "Bus stop".to_owned(),
            symbol: '🚌',
            style: Style::default(),
        },
    ])
}

/// Helper structure for the `Images` plugin.
pub struct ImagesPluginData {
    pub texture: Texture,
    pub angle: f32,
    pub x_scale: f32,
    pub y_scale: f32,
}

impl ImagesPluginData {
    pub fn new(egui_ctx: egui::Context) -> Self {
        Self {
            texture: Texture::from_color_image(egui::ColorImage::example(), &egui_ctx),
            angle: 0.0,
            x_scale: 1.0,
            y_scale: 1.0,
        }
    }
}

/// Creates a built-in `Images` plugin with an example image.
pub fn images(images_plugin_data: &mut ImagesPluginData) -> impl Plugin {
    Images::new(vec![{
        let mut image = Image::new(images_plugin_data.texture.clone(), places::wroclavia());
        image.scale(images_plugin_data.x_scale, images_plugin_data.y_scale);
        image.angle(images_plugin_data.angle.to_radians());
        image
    }])
}

/// Sample map plugin which draws custom stuff on the map.
pub struct CustomShapes {}

impl Plugin for CustomShapes {
    fn draw(&self, response: &Response, painter: Painter, projector: &Projector) {
        // Position of the point we want to put our shapes.
        let position = places::capitol();

        // Project it into the position on the screen.
        let position = projector.project(position).to_pos2();

        let radius = 30.;

        let hovered = response
            .hover_pos()
            .map(|hover_pos| hover_pos.distance(position) < radius)
            .unwrap_or(false);

        painter.circle_filled(
            position,
            radius,
            Color32::BLACK.gamma_multiply(if hovered { 0.5 } else { 0.2 }),
        );
    }
}
