use crate::plugins::ImagesPluginData;

use crate::Provider;
use egui::{Align2, RichText, Ui, Window};
use walkers::{providers::Attribution, MapMemory, Position};

pub fn acknowledge(ui: &Ui, attribution: Attribution) {
    Window::new("Acknowledge")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::LEFT_TOP, [10., 10.])
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                if let Some(logo) = attribution.logo_light {
                    ui.add(egui::Image::new(logo).max_height(30.0).max_width(80.0));
                }
                ui.hyperlink_to(attribution.text, attribution.url);
            });
        });
}

pub fn controls(
    ui: &Ui,
    selected_provider: &mut Provider,
    possible_providers: &mut dyn Iterator<Item = &Provider>,
    image: &mut ImagesPluginData,
) {
    Window::new("Satellite")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::RIGHT_TOP, [-10., 10.])
        .fixed_size([150., 150.])
        .show(ui.ctx(), |ui| {
            ui.collapsing("Map", |ui| {
                egui::ComboBox::from_label("Tile Provider")
                    .selected_text(format!("{:?}", selected_provider))
                    .show_ui(ui, |ui| {
                        for p in possible_providers {
                            ui.selectable_value(selected_provider, *p, format!("{:?}", p));
                        }
                    });
            });

            ui.collapsing("Images plugin", |ui| {
                ui.add(egui::Slider::new(&mut image.angle, 0.0..=360.0).text("Rotate"));
                ui.add(egui::Slider::new(&mut image.x_scale, 0.1..=3.0).text("Scale X"));
                ui.add(egui::Slider::new(&mut image.y_scale, 0.1..=3.0).text("Scale Y"));
            });
        });
}

/// Simple GUI to zoom in and out.
pub fn zoom(ui: &Ui, map_memory: &mut MapMemory) {
    Window::new("Map")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::LEFT_BOTTOM, [10., -10.])
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                if ui.button(RichText::new("➕").heading()).clicked() {
                    let _ = map_memory.zoom_in();
                }

                if ui.button(RichText::new("➖").heading()).clicked() {
                    let _ = map_memory.zoom_out();
                }
            });
        });
}

/// When map is "detached", show a windows with an option to go back to my position.
pub fn go_to_my_position(ui: &Ui, map_memory: &mut MapMemory) {
    if let Some(position) = map_memory.detached() {
        Window::new("Center")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(Align2::RIGHT_BOTTOM, [-10., -10.])
            .show(ui.ctx(), |ui| {
                ui.label("map center: ");
                ui.label(format!("{:.04} {:.04}", position.lon(), position.lat()));
                if ui
                    .button(RichText::new("go to the starting point").heading())
                    .clicked()
                {
                    map_memory.follow_my_position();
                }
            });
    }
}

pub fn show_my_position(ui: &Ui, my_position: &Position) {
    Window::new("My Position")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::CENTER_BOTTOM, [0., -10.])
        .show(ui.ctx(), |ui| {
            ui.label(format!(
                "{:.04} {:.04}",
                my_position.lon(),
                my_position.lat()
            ))
            .on_hover_text("my position");
        });
}
