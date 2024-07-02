mod local_tiles;
mod places;
mod plugins;
mod windows;

use std::collections::HashMap;

use crate::plugins::ImagesPluginData;
use egui::Context;
use walkers::{HttpOptions, HttpTiles, Map, MapMemory, Tiles};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Provider {
    OpenStreetMap,
    Geoportal,
    MapboxStreets,
    MapboxSatellite,
    LocalTiles,
    Custom(String),
}

fn http_options() -> HttpOptions {
    HttpOptions {
        // Not sure where to put cache on Android, so it will be disabled for now.
        cache: if cfg!(target_os = "android") || std::env::var("NO_HTTP_CACHE").is_ok() {
            None
        } else {
            Some(".cache".into())
        },
        ..Default::default()
    }
}

fn providers(egui_ctx: Context) -> HashMap<Provider, Box<dyn Tiles + Send>> {
    let mut providers: HashMap<Provider, Box<dyn Tiles + Send>> = HashMap::default();

    providers.insert(
        Provider::OpenStreetMap,
        Box::new(HttpTiles::with_options(
            walkers::sources::OpenStreetMap,
            http_options(),
            egui_ctx.to_owned(),
        )),
    );

    providers.insert(
        Provider::Geoportal,
        Box::new(HttpTiles::with_options(
            walkers::sources::Geoportal,
            http_options(),
            egui_ctx.to_owned(),
        )),
    );

    providers.insert(
        Provider::LocalTiles,
        Box::new(local_tiles::LocalTiles::new(egui_ctx.to_owned())),
    );

    // Pass in a mapbox access token at compile time. May or may not be what you want to do,
    // potentially loading it from application settings instead.
    let mapbox_access_token = std::option_env!("MAPBOX_ACCESS_TOKEN");

    // We only show the mapbox map if we have an access token
    if let Some(token) = mapbox_access_token {
        providers.insert(
            Provider::MapboxStreets,
            Box::new(HttpTiles::with_options(
                walkers::sources::Mapbox {
                    style: walkers::sources::MapboxStyle::Streets,
                    access_token: token.to_string(),
                    high_resolution: false,
                },
                http_options(),
                egui_ctx.to_owned(),
            )),
        );
        providers.insert(
            Provider::MapboxSatellite,
            Box::new(HttpTiles::with_options(
                walkers::sources::Mapbox {
                    style: walkers::sources::MapboxStyle::Satellite,
                    access_token: token.to_string(),
                    high_resolution: true,
                },
                http_options(),
                egui_ctx.to_owned(),
            )),
        );
    }

    providers
}

pub struct MyApp {
    providers: HashMap<Provider, Box<dyn Tiles + Send>>,
    selected_provider: Provider,
    map_memory: MapMemory,
    images_plugin_data: ImagesPluginData,
    click_watcher: plugins::ClickWatcher,
}

impl MyApp {
    pub fn new(egui_ctx: Context) -> Self {
        egui_extras::install_image_loaders(&egui_ctx);

        // Data for the `images` plugin showcase.
        let images_plugin_data = ImagesPluginData::new(egui_ctx.to_owned());

        Self {
            providers: providers(egui_ctx.to_owned()),
            selected_provider: Provider::OpenStreetMap,
            map_memory: MapMemory::default(),
            images_plugin_data,
            click_watcher: Default::default(),
        }
    }
    pub fn with_provider(
        mut self,
        name: Provider,
        new_provider: Box<dyn TilesManager + Send>,
    ) -> Self {
        self.providers.insert(name, new_provider);

        self
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

                let tiles = self
                    .providers
                    .get_mut(&self.selected_provider)
                    .unwrap()
                    .as_mut();
                let attribution = tiles.attribution();

                // In egui, widgets are constructed and consumed in each frame.
                let map = Map::new(Some(tiles), &mut self.map_memory, my_position);

                // Optionally, plugins can be attached.
                let map = map
                    .with_plugin(plugins::places())
                    .with_plugin(plugins::images(&mut self.images_plugin_data))
                    .with_plugin(plugins::CustomShapes {})
                    .with_plugin(&mut self.click_watcher);

                // Draw the map widget.
                ui.add(map);

                // Draw utility windows.
                {
                    use windows::*;

                    zoom(ui, &mut self.map_memory);
                    go_to_my_position(ui, &mut self.map_memory);
                    self.click_watcher.show_position(ui);
                    controls(
                        ui,
                        &mut self.selected_provider,
                        &mut self.providers.keys(),
                        &mut self.images_plugin_data,
                    );
                    acknowledge(ui, attribution);
                }
            });
    }
}
