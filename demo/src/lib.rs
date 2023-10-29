use egui::{Align2, Context, Painter, Shape};
use walkers::{
    extras::{Image, Images, Place, Places, Style, Texture},
    Map, MapMemory, Plugin, Projector, Tiles,
};

pub struct MyApp {
    tiles: Tiles,
    geoportal_tiles: Tiles,
    map_memory: MapMemory,
    satellite: bool,
    texture: Texture,
    rotate: f32,
    x_scale: f32,
    y_scale: f32,
}

impl MyApp {
    pub fn new(egui_ctx: Context) -> Self {
        let texture = Texture::new(
            egui_ctx.to_owned(),
            "Wroclavia",
            egui::ColorImage::example(),
        );

        Self {
            tiles: Tiles::new(walkers::providers::OpenStreetMap, egui_ctx.to_owned()),
            geoportal_tiles: Tiles::new(walkers::providers::Geoportal, egui_ctx),
            map_memory: MapMemory::default(),
            satellite: false,
            texture,
            rotate: 0.0,
            x_scale: 1.0,
            y_scale: 1.0,
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

                // Select either OSM standard map or satellite.
                let tiles = if self.satellite {
                    &mut self.geoportal_tiles
                } else {
                    &mut self.tiles
                };

                let attribution = tiles.attribution();

                // In egui, widgets are constructed and consumed in each frame.
                let map = Map::new(Some(tiles), &mut self.map_memory, my_position);

                // Optionally, a plugin which draw custom stuff on the map can be attached.
                let map = map
                    .with_plugin(Places::new(vec![
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
                    ]))
                    .with_plugin(Images::new(vec![Image {
                        position: places::wroclavia(),
                        texture: self.texture.clone(),
                    }]));
                // Draw the map widget.
                ui.add(map);

                // Draw utility windows.
                {
                    use windows::*;

                    zoom(ui, &mut self.map_memory);
                    go_to_my_position(ui, &mut self.map_memory);
                    controls(
                        ui,
                        &mut self.satellite,
                        &mut self.texture,
                        &mut self.rotate,
                        &mut self.x_scale,
                        &mut self.y_scale,
                    );
                    acknowledge(ui, &attribution);
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

    /// Shopping center, and main intercity bus station.
    pub fn wroclavia() -> Position {
        Position::new(17.03471, 51.09648)
    }
}

/// Sample map plugin which draws custom stuff on the map.
struct CustomShapes {}

impl Plugin for CustomShapes {
    fn draw(&self, painter: Painter, projector: &Projector) {
        // Position of the point we want to put our shapes.
        let position = places::dworcowa_bus_stop();

        // Project it into the position on the screen.
        let screen_position = projector.project(position);

        // Context is a central part of egui. Among other things, it holds styles and fonts.
        let ctx = painter.ctx();

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
}

mod windows {
    use egui::{Align2, RichText, Ui, Window};
    use walkers::extras::Texture;
    use walkers::{providers::Attribution, Center, MapMemory};

    pub fn acknowledge(ui: &Ui, attribution: &Attribution) {
        Window::new("Acknowledge")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(Align2::LEFT_TOP, [10., 10.])
            .show(ui.ctx(), |ui| {
                ui.hyperlink_to(attribution.text, attribution.url);
            });
    }

    pub fn controls(
        ui: &Ui,
        satellite: &mut bool,
        texture: &mut Texture,
        rotate: &mut f32,
        x_scale: &mut f32,
        y_scale: &mut f32,
    ) {
        Window::new("Satellite")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(Align2::RIGHT_TOP, [-10., 10.])
            .fixed_size([150., 150.])
            .show(ui.ctx(), |ui| {
                ui.checkbox(satellite, "satellite view");
                ui.add(egui::Slider::new(rotate, 0.0..=360.0).text("Rotate"));
                ui.add(egui::Slider::new(x_scale, 0.1..=3.0).text("Scale width"));
                ui.add(egui::Slider::new(y_scale, 0.1..=3.0).text("Scale heigth"));
                texture.scale(*x_scale, *y_scale);
                texture.rotate((*rotate).to_radians());
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
        if let Some(position) = map_memory.center_mode.detached() {
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
