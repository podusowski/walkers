use egui::{Align2, Context, Painter, RichText, Shape, Ui, Vec2, Window};
use walkers::{Map, MapMemory, Position, PositionExt, Tiles};

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    eframe::run_native(
        "OpenStreetMap",
        Default::default(),
        Box::new(|cc| Box::new(Osm::new(cc.egui_ctx.clone()))),
    )
}

struct Osm {
    tiles: Tiles,
    map_memory: MapMemory,
}

impl Osm {
    fn new(egui_ctx: Context) -> Self {
        let mut map_memory = MapMemory::default();
        map_memory.osm = true;
        Self {
            tiles: Tiles::new(egui_ctx),
            map_memory,
        }
    }
}

/// Main train station of the city of Wrocław.
/// https://en.wikipedia.org/wiki/Wroc%C5%82aw_G%C5%82%C3%B3wny_railway_station
fn wroclaw_glowny() -> Position {
    Position::new(17.03664, 51.09916)
}

/// Taking a public bus (line 106) is probably the cheapest option to get from
/// the train station to the airport.
/// https://www.wroclaw.pl/en/how-and-where-to-buy-public-transport-tickets-in-wroclaw
fn dworcowa_bus_stop() -> Position {
    Position::new(17.03940, 51.10005)
}

/// Shows how to draw various things in the map.
fn draw_custom_shapes(ui: &Ui, painter: Painter, map_memory: &MapMemory, my_position: Position) {
    // Geographical position of the point we want to put our shapes.
    let position = dworcowa_bus_stop();

    // Turn that into a flat, mercator projection.
    let projected_position = position.project_with_zoom(*map_memory.zoom);

    // We also need to know where the map center is.
    let map_center_projected_position = map_memory
        .center_mode
        .position(my_position)
        .project_with_zoom(*map_memory.zoom);

    // From the two points above we can calculate the actual point on the screen.
    let screen_position = painter.clip_rect().center()
        + (Vec2::from(projected_position) - Vec2::from(map_center_projected_position));

    // Now we can just use Painter to draw stuff.
    let background = |text: &Shape| {
        Shape::rect_filled(
            text.visual_bounding_rect().expand(5.),
            5.,
            ui.visuals().extreme_bg_color,
        )
    };

    let text = ui.fonts(|fonts| {
        Shape::text(
            fonts,
            screen_position,
            Align2::LEFT_CENTER,
            "⬉ Here you can board the 106 line\nwhich goes to the airport.",
            Default::default(),
            ui.visuals().text_color(),
        )
    });
    painter.add(background(&text));
    painter.add(text);
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

            // Typically this would be a GPS acquired position which is tracked by the map.
            let my_position = wroclaw_glowny();

            // Draw the actual map.
            let response = ui.add(Map::new(
                &mut self.tiles,
                &mut self.map_memory,
                wroclaw_glowny(),
            ));

            // Draw custom shapes.
            let painter = ui.painter().with_clip_rect(response.rect);
            draw_custom_shapes(ui, painter, &self.map_memory, my_position);

            // Simple GUI to zoom in and out.
            Window::new("Map")
                .collapsible(false)
                .resizable(false)
                .title_bar(false)
                .anchor(Align2::LEFT_BOTTOM, [10., -10.])
                .show(ui.ctx(), |ui| {
                    ui.label(format!("zoom: {}", *self.map_memory.zoom));
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("➕").heading()).clicked() {
                            let _ = self.map_memory.zoom.zoom_in();
                        }

                        if ui.button(RichText::new("➖").heading()).clicked() {
                            let _ = self.map_memory.zoom.zoom_out();
                        }
                    });
                });
        });
    }
}
