use egui::{Color32, Response, Ui};
use walkers::{
    extras::{
        GroupedPlaces, Image, LabeledSymbol, LabeledSymbolGroup, LabeledSymbolGroupStyle,
        LabeledSymbolStyle, Places, Texture,
    },
    MapMemory, Plugin, Position, Projector,
};

use crate::places;

/// Creates a built-in [`GroupedPlaces`] plugin populated with some predefined places.
pub fn places() -> impl Plugin {
    GroupedPlaces::new(
        vec![
            LabeledSymbol {
                position: places::wroclaw_glowny(),
                label: "Wrocław Główny\ntrain station".to_owned(),
                symbol: '🚆',
                style: LabeledSymbolStyle::default(),
            },
            LabeledSymbol {
                position: places::dworcowa_bus_stop(),
                label: "Bus stop".to_owned(),
                symbol: '🚌',
                style: LabeledSymbolStyle::default(),
            },
        ],
        LabeledSymbolGroup {
            style: LabeledSymbolGroupStyle::default(),
        },
    )
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
    Places::new(vec![{
        let mut image = Image::new(images_plugin_data.texture.clone(), places::wroclavia());
        image.scale(images_plugin_data.x_scale, images_plugin_data.y_scale);
        image.angle(images_plugin_data.angle.to_radians());
        image
    }])
}

/// Sample map plugin which draws custom stuff on the map.
pub struct CustomShapes {}

impl Plugin for CustomShapes {
    fn run(
        self: Box<Self>,
        ui: &mut Ui,
        response: &Response,
        projector: &Projector,
        _map_memory: &MapMemory,
    ) {
        // Position of the point we want to put our shapes.
        let position = places::capitol();

        // Compute pixel radius for a 100-meter circle.
        let radius = 100.0 * projector.scale_pixel_per_meter(position);

        // Project it into the position on the screen.
        let position = projector.project(position).to_pos2();

        let hovered = response
            .hover_pos()
            .map(|hover_pos| hover_pos.distance(position) < radius)
            .unwrap_or(false);

        ui.painter().circle_filled(
            position,
            radius,
            Color32::BLACK.gamma_multiply(if hovered { 0.5 } else { 0.2 }),
        );
    }
}

#[derive(Default, Clone)]
pub struct ClickWatcher {
    pub clicked_at: Option<Position>,
}

impl ClickWatcher {
    pub fn show_position(&self, ui: &egui::Ui) {
        if let Some(clicked_at) = self.clicked_at {
            egui::Window::new("Clicked Position")
                .collapsible(false)
                .resizable(false)
                .title_bar(false)
                .anchor(egui::Align2::CENTER_BOTTOM, [0., -10.])
                .show(ui.ctx(), |ui| {
                    ui.label(format!("{:.04} {:.04}", clicked_at.x(), clicked_at.y()))
                        .on_hover_text("last clicked position");
                });
        }
    }
}

impl Plugin for &mut ClickWatcher {
    fn run(
        self: Box<Self>,
        ui: &mut Ui,
        response: &Response,
        projector: &Projector,
        _map_memory: &MapMemory,
    ) {
        if !response.changed() && response.clicked_by(egui::PointerButton::Primary) {
            self.clicked_at = response
                .interact_pointer_pos()
                .map(|p| projector.unproject(p.to_vec2()));
        }

        if let Some(position) = self.clicked_at {
            ui.painter()
                .circle_filled(projector.project(position).to_pos2(), 5.0, Color32::BLUE);
        }
    }
}
