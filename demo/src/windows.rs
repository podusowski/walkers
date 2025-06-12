use crate::MyApp;
use egui::{Align2, ComboBox, Image, RichText, Slider, Ui, Window};
use walkers::{sources::Attribution, MapMemory};

pub fn acknowledge(ui: &Ui, attributions: Vec<Attribution>) {
    Window::new("Acknowledge")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::LEFT_TOP, [10., 10.])
        .show(ui.ctx(), |ui| {
            ui.label("map provided by");
            for attribution in attributions {
                ui.horizontal(|ui| {
                    if let Some(logo) = attribution.logo_light {
                        ui.add(Image::new(logo).max_height(30.0).max_width(80.0));
                    }
                    ui.hyperlink_to(attribution.text, attribution.url);
                });
            }
            ui.label("viewed in ");
            ui.hyperlink_to("Walkers", "https://github.com/podusowski/walkers");
        });
}

pub fn controls(app: &mut MyApp, ui: &Ui, http_stats: Vec<walkers::HttpStats>) {
    Window::new("Controls")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::RIGHT_TOP, [-10., 10.])
        .fixed_size([150., 150.])
        .show(ui.ctx(), |ui| {
            ui.collapsing("Map", |ui| {
                ComboBox::from_label("Tile Provider")
                    .selected_text(format!("{:?}", app.selected_provider))
                    .show_ui(ui, |ui| {
                        for p in app.providers.keys() {
                            ui.selectable_value(&mut app.selected_provider, *p, format!("{:?}", p));
                        }
                    });

                ui.checkbox(&mut app.zoom_with_ctrl, "Zoom with Ctrl");
            });

            ui.collapsing("HTTP statistics", |ui| {
                for http_stats in http_stats {
                    ui.label(format!(
                        "{:?} requests in progress: {}",
                        app.selected_provider, http_stats.in_progress
                    ));
                }
            });

            ui.collapsing("Images plugin", |ui| {
                ui.add(Slider::new(&mut app.images_plugin_data.angle, 0.0..=360.0).text("Rotate"));
                ui.add(Slider::new(&mut app.images_plugin_data.x_scale, 0.1..=3.0).text("Scale X"));
                ui.add(Slider::new(&mut app.images_plugin_data.y_scale, 0.1..=3.0).text("Scale Y"));
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
                ui.label(format!(
                    "center at {:.04} {:.04}",
                    position.x(),
                    position.y()
                ));
                if ui
                    .button(RichText::new("go to the starting point").heading())
                    .clicked()
                {
                    map_memory.follow_my_position();
                }
            });
    }
}
