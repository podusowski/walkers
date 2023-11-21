use egui::{Color32, Context, Painter, Response};
use walkers::{
    extras::{Image, Images, Place, Places, Style, Texture},
    Map, MapMemory, Plugin, Projector, Tiles,
};

pub struct ImagesPluginData {
    texture: Texture,
    angle: f32,
    x_scale: f32,
    y_scale: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SelectedProvider {
    OpenStreetMap,
    Geoportal,
    MapboxStreets,
    MapboxSatellite,
}

pub struct MyApp {
    tiles: Tiles,
    geoportal_tiles: Tiles,
    mapbox_tiles_streets: Option<Tiles>,
    mapbox_tiles_satellite: Option<Tiles>,
    map_memory: MapMemory,
    selected_tile_provider: SelectedProvider,
    images_plugin_data: ImagesPluginData,
}

impl MyApp {
    pub fn new(egui_ctx: Context) -> Self {
        egui_extras::install_image_loaders(&egui_ctx);

        // Data for the `images` plugin showcase.
        let images_plugin_data = ImagesPluginData {
            texture: Texture::from_color_image(egui::ColorImage::example(), &egui_ctx),
            angle: 0.0,
            x_scale: 1.0,
            y_scale: 1.0,
        };

        // Pass in a mapbox access token at compile time. May or may not be what you want to do,
        // potentially loading it from application settings instead.
        let mapbox_access_token = std::option_env!("MAPBOX_ACCESS_TOKEN");

        // We only show the mapbox map if we have an access token
        let mapbox_streets = mapbox_access_token.map(|t| walkers::providers::Mapbox {
            style: walkers::providers::MapboxStyle::Streets,
            access_token: t.to_string(),
            high_resolution: false,
        });

        let mapbox_satellite = mapbox_access_token.map(|t| walkers::providers::Mapbox {
            style: walkers::providers::MapboxStyle::Satellite,
            access_token: t.to_string(),
            high_resolution: true,
        });

        Self {
            tiles: Tiles::new(walkers::providers::OpenStreetMap, egui_ctx.to_owned()),
            geoportal_tiles: Tiles::new(walkers::providers::Geoportal, egui_ctx.to_owned()),
            mapbox_tiles_streets: mapbox_streets.map(|p| Tiles::new(p, egui_ctx.to_owned())),
            mapbox_tiles_satellite: mapbox_satellite.map(|p| Tiles::new(p, egui_ctx.to_owned())),
            map_memory: MapMemory::default(),
            selected_tile_provider: SelectedProvider::OpenStreetMap,
            images_plugin_data,
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

                // Select tile provider
                let tiles = match self.selected_tile_provider {
                    SelectedProvider::OpenStreetMap => &mut self.tiles,
                    SelectedProvider::Geoportal => &mut self.geoportal_tiles,
                    SelectedProvider::MapboxStreets => self.mapbox_tiles_streets.as_mut().unwrap(),
                    SelectedProvider::MapboxSatellite => {
                        self.mapbox_tiles_satellite.as_mut().unwrap()
                    }
                };

                let attribution = tiles.attribution();

                // In egui, widgets are constructed and consumed in each frame.
                let map = Map::new(Some(tiles), &mut self.map_memory, my_position);

                // Optionally, plugins can be attached.
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
                    .with_plugin(Images::new(vec![{
                        let mut image = Image::new(
                            self.images_plugin_data.texture.clone(),
                            places::wroclavia(),
                        );
                        image.scale(
                            self.images_plugin_data.x_scale,
                            self.images_plugin_data.y_scale,
                        );
                        image.angle(self.images_plugin_data.angle.to_radians());
                        image
                    }]))
                    .with_plugin(CustomShapes {});

                // Draw the map widget.
                ui.add(map);

                // Draw utility windows.
                {
                    use windows::*;

                    let mut possible_providers =
                        vec![SelectedProvider::OpenStreetMap, SelectedProvider::Geoportal];
                    if self.mapbox_tiles_streets.is_some() {
                        possible_providers.extend([
                            SelectedProvider::MapboxStreets,
                            SelectedProvider::MapboxSatellite,
                        ]);
                    }

                    zoom(ui, &mut self.map_memory);
                    go_to_my_position(ui, &mut self.map_memory);
                    controls(
                        ui,
                        &mut self.selected_tile_provider,
                        &possible_providers,
                        &mut self.images_plugin_data,
                    );
                    acknowledge(ui, attribution);
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
        Position::from_lon_lat(17.03664, 51.09916)
    }

    /// Taking a public bus (line 106) is probably the cheapest option to get from
    /// the train station to the airport.
    /// https://www.wroclaw.pl/en/how-and-where-to-buy-public-transport-tickets-in-wroclaw
    pub fn dworcowa_bus_stop() -> Position {
        Position::from_lon_lat(17.03940, 51.10005)
    }

    /// Musical Theatre Capitol.
    /// https://www.teatr-capitol.pl/
    pub fn capitol() -> Position {
        Position::from_lon_lat(17.03018, 51.10073)
    }

    /// Shopping center, and the main intercity bus station.
    pub fn wroclavia() -> Position {
        Position::from_lon_lat(17.03471, 51.09648)
    }
}

/// Sample map plugin which draws custom stuff on the map.
struct CustomShapes {}

impl Plugin for CustomShapes {
    fn draw(&self, _response: &Response, painter: Painter, projector: &Projector) {
        // Position of the point we want to put our shapes.
        let position = places::capitol();

        // Project it into the position on the screen.
        let screen_position = projector.project(position);

        painter.circle_filled(
            screen_position.to_pos2(),
            30.,
            Color32::BLACK.gamma_multiply(0.5),
        );
    }
}

mod windows {
    use super::{ImagesPluginData, SelectedProvider};
    use egui::{Align2, RichText, Ui, Window};
    use walkers::{providers::Attribution, MapMemory};

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
        selected_provider: &mut SelectedProvider,
        possible_providers: &[SelectedProvider],
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
                    ui.label(format!("{:.04} {:.04}", position.lon(), position.lat()));
                    if ui
                        .button(RichText::new("go to my (fake) position ").heading())
                        .clicked()
                    {
                        map_memory.follow_my_position();
                    }
                });
        }
    }
}
