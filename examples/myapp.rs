use egui::{Align2, Context, Painter, Shape};
use walkers::{Map, MapMemory, Projector, Tiles};

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    eframe::run_native(
        "MyApp",
        Default::default(),
        Box::new(|cc| Box::new(MyApp::new(cc.egui_ctx.clone()))),
    )
}

struct MyApp {
    tiles: Tiles,
    geoportal_tiles: Tiles,
    map_memory: MapMemory,
}

impl MyApp {
    fn new(egui_ctx: Context) -> Self {
        Self {
            tiles: Tiles::new(walkers::providers::openstreetmap, egui_ctx.to_owned()),
            geoportal_tiles: Tiles::new(walkers::providers::geoportal, egui_ctx),
            map_memory: MapMemory::default(),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let rimless = egui::Frame {
            fill: ctx.style().visuals.panel_fill,
            ..Default::default()
        };

        egui::CentralPanel::default()
            .frame(rimless)
            .show(ctx, |ui| {
                // Typically this would be a GPS acquired position which is tracked by the map.
                let my_position = places::wroclaw_glowny();

                // In egui, widgets are constructed and consumed in each frame.
                let map = Map::new(Some(&mut self.tiles), &mut self.map_memory, my_position);

                // Optionally, a function which draw custom stuff on the map can be attached.
                let ctx_clone = ctx.clone();
                let map = map.with_drawer(move |painter, project| {
                    draw_custom_shapes(ctx_clone.clone(), painter, project);
                });

                // Draw the map widget.
                ui.add(map);

                // Draw utility windows.
                {
                    use windows::*;

                    zoom(ui, &mut self.map_memory);
                    go_to_my_position(ui, &mut self.map_memory);

                    orthophotomap(
                        ui,
                        &mut self.geoportal_tiles,
                        &mut self.map_memory,
                        my_position,
                    );

                    acknowledge(ui);
                }
            });
    }
}

mod places {
    //! Few common places in the city of Wrocław, used in the example app.
    use walkers::Position;

    /// Main train station of the city of Wrocław.
    /// https://en.wikipedia.org/wiki/Wroc%C5%82aw_G%C5%82%C3%B3wny_railway_station
    pub fn wroclaw_glowny() -> Position {
        Position::new(17.03664, 51.09916)
    }

    /// Taking a public bus (line 106) is probably the cheapest option to get from
    /// the train station to the airport.
    /// https://www.wroclaw.pl/en/how-and-where-to-buy-public-transport-tickets-in-wroclaw
    pub fn dworcowa_bus_stop() -> Position {
        Position::new(17.03940, 51.10005)
    }
}

/// Shows how to draw various things in the map.
fn draw_custom_shapes(ctx: Context, painter: Painter, projector: &Projector) {
    // Position of the point we want to put our shapes.
    let position = places::dworcowa_bus_stop();

    // Project it into the position on the screen.
    let screen_position = projector.project(position);

    // Now we can just use Painter to draw stuff.
    let background = |text: &Shape| {
        Shape::rect_filled(
            text.visual_bounding_rect().expand(5.),
            5.,
            ctx.style().visuals.extreme_bg_color,
        )
    };

    let text = ctx.fonts(|fonts| {
        Shape::text(
            fonts,
            screen_position.to_pos2(),
            Align2::LEFT_CENTER,
            "⬉ Here you can board the 106 line\nwhich goes to the airport.",
            Default::default(),
            ctx.style().visuals.text_color(),
        )
    });
    painter.add(background(&text));
    painter.add(text);
}

mod windows {
    use egui::{Align2, RichText, Ui, Window};
    use walkers::{Center, Map, MapMemory, Position, Tiles};

    pub fn acknowledge(ui: &Ui) {
        Window::new("Acknowledge")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(Align2::LEFT_TOP, [10., 10.])
            .fixed_size([150., 150.])
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.label("following map uses data from");
                    ui.hyperlink("https://www.openstreetmap.org");
                });
            });
    }

    pub fn orthophotomap(
        ui: &Ui,
        tiles: &mut Tiles,
        map_memory: &mut MapMemory,
        my_position: Position,
    ) {
        Window::new("Orthophotomap")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(Align2::RIGHT_TOP, [-10., 10.])
            .fixed_size([150., 150.])
            .show(ui.ctx(), |ui| {
                ui.add(Map::new(Some(tiles), map_memory, my_position));
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
                        let _ = map_memory.zoom.zoom_in();
                    }

                    if ui.button(RichText::new("➖").heading()).clicked() {
                        let _ = map_memory.zoom.zoom_out();
                    }
                });
            });
    }

    /// When map is "detached", show a windows with an option to go back to my position.
    pub fn go_to_my_position(ui: &Ui, map_memory: &mut MapMemory) {
        if let Center::Exact(position) = map_memory.center_mode {
            Window::new("Center")
                .collapsible(false)
                .resizable(false)
                .title_bar(false)
                .anchor(Align2::RIGHT_BOTTOM, [-10., -10.])
                .show(ui.ctx(), |ui| {
                    ui.label(format!("{:.04} {:.04}", position.x(), position.y()));
                    if ui
                        .button(RichText::new("go to my (fake) position ").heading())
                        .clicked()
                    {
                        map_memory.center_mode = Center::MyPosition;
                    }
                });
        }
    }
}
