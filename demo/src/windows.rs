use crate::MyApp;
use egui::{Align2, ComboBox, Image, RichText, Ui, Window};
use walkers::{MapMemory, sources::Attribution};

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

pub fn controls(
    app: &mut MyApp,
    ui: &Ui,
    http_stats: Vec<walkers::Stats>,
    frame: &mut eframe::Frame,
) {
    Window::new("Controls")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::RIGHT_TOP, [-10., 10.])
        .fixed_size([150., 150.])
        .show(ui.ctx(), |ui| {
            ui.heading("Map");

            ComboBox::from_label("Tile Provider")
                .selected_text(app.providers.selected.to_owned())
                .show_ui(ui, |ui| {
                    for p in app.providers.available.keys() {
                        ui.selectable_value(&mut app.providers.selected, p.clone(), p);
                    }
                });

            #[cfg(feature = "pmtiles")]
            if !app.providers.have_some_pmtiles {
                ui.label("No .pmtiles files found in the current directory. Go to");
                ui.hyperlink("https://docs.protomaps.com/guide/getting-started");
                ui.label(" to see how to fetch some.");
            }

            ui.add_space(10.0);
            ui.heading("Settings");

            ui.checkbox(&mut app.zoom_with_ctrl, "Zoom with Ctrl");

            ui.add_space(10.0);
            ui.heading("Debug");

            if app.map_memory.animating() {
                ui.label("Map is animating");
            } else {
                ui.label("Map is not animating");
            }

            for http_stats in http_stats {
                ui.label(format!(
                    "{:?} requests in progress: {}",
                    app.providers.selected, http_stats.in_progress
                ));
            }

            if let Some(cpu_usage) = frame.info().cpu_usage {
                ui.label(format!("CPU usage: {:.2}ms", cpu_usage * 1000.0));
            }
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
