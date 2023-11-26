mod places;
mod plugins;
mod windows;

use std::collections::HashMap;

use crate::plugins::ImagesPluginData;
use egui::Context;
use walkers::{Map, MapMemory, Tiles};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectedProvider {
    OpenStreetMap,
    Geoportal,
    MapboxStreets,
    MapboxSatellite,
}

fn providers(egui_ctx: Context) -> HashMap<SelectedProvider, Tiles> {
    let mut providers = HashMap::default();

    providers.insert(
        SelectedProvider::OpenStreetMap,
        Tiles::new(walkers::providers::OpenStreetMap, egui_ctx.to_owned()),
    );

    providers.insert(
        SelectedProvider::Geoportal,
        Tiles::new(walkers::providers::Geoportal, egui_ctx.to_owned()),
    );

    // Pass in a mapbox access token at compile time. May or may not be what you want to do,
    // potentially loading it from application settings instead.
    let mapbox_access_token = std::option_env!("MAPBOX_ACCESS_TOKEN");

    // We only show the mapbox map if we have an access token
    if let Some(token) = mapbox_access_token {
        providers.insert(
            SelectedProvider::MapboxStreets,
            Tiles::new(
                walkers::providers::Mapbox {
                    style: walkers::providers::MapboxStyle::Streets,
                    access_token: token.to_string(),
                    high_resolution: false,
                },
                egui_ctx.to_owned(),
            ),
        );

        providers.insert(
            SelectedProvider::MapboxSatellite,
            Tiles::new(
                walkers::providers::Mapbox {
                    style: walkers::providers::MapboxStyle::Satellite,
                    access_token: token.to_string(),
                    high_resolution: true,
                },
                egui_ctx.to_owned(),
            ),
        );
    }

    providers
}

pub struct MyApp {
    providers: HashMap<SelectedProvider, Tiles>,
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
        let images_plugin_data = ImagesPluginData::new(egui_ctx.to_owned());

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
            providers: providers(egui_ctx.to_owned()),
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
                let tiles = self
                    .providers
                    .get_mut(&self.selected_tile_provider)
                    .unwrap();

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
                        &mut self.providers.keys(),
                        &mut self.images_plugin_data,
                    );
                    acknowledge(ui, attribution);
                }
            });
    }
}
