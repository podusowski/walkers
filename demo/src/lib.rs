mod places;
mod plugins;
mod windows;

use std::collections::HashMap;

use crate::plugins::ImagesPluginData;
use egui::Context;
use walkers::{Map, MapMemory, Tiles, TilesManager};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
    OpenStreetMap,
    Geoportal,
    MapboxStreets,
    MapboxSatellite,
}

fn providers(egui_ctx: Context) -> HashMap<Provider, Box<dyn TilesManager + Send>> {
    let mut providers: HashMap<Provider, Box<dyn TilesManager + Send>> = HashMap::default();

    providers.insert(
        Provider::OpenStreetMap,
        Box::new(Tiles::new(
            walkers::providers::OpenStreetMap,
            egui_ctx.to_owned(),
        )),
    );

    providers.insert(
        Provider::Geoportal,
        Box::new(Tiles::new(
            walkers::providers::Geoportal,
            egui_ctx.to_owned(),
        )),
    );

    // Pass in a mapbox access token at compile time. May or may not be what you want to do,
    // potentially loading it from application settings instead.
    let mapbox_access_token = std::option_env!("MAPBOX_ACCESS_TOKEN");

    // We only show the mapbox map if we have an access token
    if let Some(token) = mapbox_access_token {
        providers.insert(
            Provider::MapboxStreets,
            Box::new(Tiles::new(
                walkers::providers::Mapbox {
                    style: walkers::providers::MapboxStyle::Streets,
                    access_token: token.to_string(),
                    high_resolution: false,
                },
                egui_ctx.to_owned(),
            )),
        );
        providers.insert(
            Provider::MapboxSatellite,
            Box::new(Tiles::new(
                walkers::providers::Mapbox {
                    style: walkers::providers::MapboxStyle::Satellite,
                    access_token: token.to_string(),
                    high_resolution: true,
                },
                egui_ctx.to_owned(),
            )),
        );
    }

    providers
}

pub struct MyApp {
    providers: HashMap<Provider, Box<dyn TilesManager + Send>>,
    selected_provider: Provider,
    map_memory: MapMemory,
    images_plugin_data: ImagesPluginData,
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
                    .with_plugin(plugins::CustomShapes {});

                // Draw the map widget.
                ui.add(map);

                // Draw utility windows.
                {
                    use windows::*;

                    zoom(ui, &mut self.map_memory);
                    go_to_my_position(ui, &mut self.map_memory);
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
