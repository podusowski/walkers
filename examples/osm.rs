use egui::{Align2, FontId, RichText, Window};
use walkers::MapMemory;

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    eframe::run_native(
        "OpenStreetMap",
        Default::default(),
        Box::new(|_cc| Box::new(Osm::new())),
    )
}

struct Osm {
    tiles: walkers::Tiles,
    map_memory: MapMemory,
}

impl Osm {
    fn new() -> Self {
        let mut map_memory = MapMemory::default();
        map_memory.osm = true;
        Self {
            tiles: walkers::Tiles::new(),
            map_memory,
        }
    }
}

impl eframe::App for Osm {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("following map uses data from");
                ui.hyperlink("https://www.openstreetmap.org");
                ui.label(", please consider donating at");
                ui.hyperlink("https://donate.openstreetmap.org/");
            });

            ui.add(walkers::Map::new(
                &mut self.tiles,
                &mut self.map_memory,
                walkers::Position::new(17.03664, 51.09916),
            ));

            Window::new("Map")
                .collapsible(false)
                .resizable(false)
                .title_bar(false)
                .anchor(Align2::LEFT_BOTTOM, [10., -10.])
                .show(ui.ctx(), |ui| {
                    ui.label(format!("current zoom: {}", *self.map_memory.zoom));
                    ui.horizontal(|ui| {
                        if ui
                            .button(RichText::new("➕").font(FontId::proportional(20.)))
                            .clicked()
                        {
                            let _ = self.map_memory.zoom.zoom_in();
                        }

                        if ui
                            .button(RichText::new("➖").font(FontId::proportional(20.)))
                            .clicked()
                        {
                            let _ = self.map_memory.zoom.zoom_out();
                        }
                    });
                });
        });
    }
}
