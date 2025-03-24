mod local_tiles;
mod places;
mod plugins;
mod windows;

use crate::plugins::ImagesPluginData;
use egui::{CentralPanel, Context, Frame};
use local_tiles::LocalTiles;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::DerefMut;
use walkers::sources::{Attribution, TileSource};
use walkers::{HttpOptions, HttpTiles, Map, MapMemory, TileId, Tiles};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
    OpenStreetMap,
    Geoportal,
    MapboxStreets,
    MapboxSatellite,
    LocalTiles,
    NOAATest,
}

enum TilesKind {
    Http(HttpTiles),
    Local(LocalTiles),
}

impl AsMut<dyn Tiles> for TilesKind {
    fn as_mut(&mut self) -> &mut (dyn Tiles + 'static) {
        match self {
            TilesKind::Http(tiles) => tiles,
            TilesKind::Local(tiles) => tiles,
        }
    }
}

impl AsRef<dyn Tiles> for TilesKind {
    fn as_ref(&self) -> &(dyn Tiles + 'static) {
        match self {
            TilesKind::Http(tiles) => tiles,
            TilesKind::Local(tiles) => tiles,
        }
    }
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

fn providers(egui_ctx: Context) -> HashMap<Provider, RefCell<TilesKind>> {
    let mut providers: HashMap<Provider, RefCell<TilesKind>> = HashMap::default();

    providers.insert(
        Provider::OpenStreetMap,
        RefCell::new(TilesKind::Http(HttpTiles::with_options(
            walkers::sources::OpenStreetMap,
            http_options(),
            egui_ctx.to_owned(),
        ))),
    );

    providers.insert(
        Provider::Geoportal,
        RefCell::new(TilesKind::Http(HttpTiles::with_options(
            walkers::sources::Geoportal,
            http_options(),
            egui_ctx.to_owned(),
        ))),
    );

    providers.insert(
        Provider::LocalTiles,
        RefCell::new(TilesKind::Local(local_tiles::LocalTiles::new(
            egui_ctx.to_owned(),
        ))),
    );

    // Pass in a mapbox access token at compile time. May or may not be what you want to do,
    // potentially loading it from application settings instead.
    let mapbox_access_token = std::option_env!("MAPBOX_ACCESS_TOKEN");

    // We only show the mapbox map if we have an access token
    if let Some(token) = mapbox_access_token {
        providers.insert(
            Provider::MapboxStreets,
            RefCell::new(TilesKind::Http(HttpTiles::with_options(
                walkers::sources::Mapbox {
                    style: walkers::sources::MapboxStyle::Streets,
                    access_token: token.to_string(),
                    high_resolution: false,
                },
                http_options(),
                egui_ctx.to_owned(),
            ))),
        );
        providers.insert(
            Provider::MapboxSatellite,
            RefCell::new(TilesKind::Http(HttpTiles::with_options(
                walkers::sources::Mapbox {
                    style: walkers::sources::MapboxStyle::Satellite,
                    access_token: token.to_string(),
                    high_resolution: true,
                },
                http_options(),
                egui_ctx.to_owned(),
            ))),
        );
    }

    providers.insert(
        Provider::NOAATest,
        RefCell::new(TilesKind::Http(HttpTiles::with_options(
            TestSource {},
            http_options(),
            egui_ctx.to_owned(),
        ))),
    );

    providers
}

pub struct MyApp {
    providers: HashMap<Provider, RefCell<TilesKind>>,
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
}

struct TestSource {}
impl TileSource for TestSource {
    fn tile_url(&self, tile_id: TileId) -> String {
        format!("http://localhost:8080/grib2.noaa_mrms_merged_composite_reflectivity_qc_CONUS/{}/{}/{}.png", tile_id.zoom, tile_id.x, tile_id.y)
    }

    fn attribution(&self) -> Attribution {
        Attribution {
            text: "NOAA",
            url: "",
            logo_light: None,
            logo_dark: None,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        CentralPanel::default().frame(Frame::NONE).show(ctx, |ui| {
            // Typically this would be a GPS acquired position which is tracked by the map.
            let my_position = places::wroclaw_glowny();

            let mut tiles = self
                .providers
                .get(&self.selected_provider)
                .unwrap()
                .borrow_mut();
            let mut overlaytiles = self
                .providers
                .get(&Provider::NOAATest)
                .unwrap()
                .borrow_mut();

            let attribution = tiles.as_ref().attribution();

            // In egui, widgets are constructed and consumed in each frame.
            let map = Map::new(
                vec![tiles.as_mut(), overlaytiles.as_mut()],
                &mut self.map_memory,
                my_position,
            );

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

                let http_stats = if let TilesKind::Http(tiles) = tiles.deref_mut() {
                    Some(tiles.stats())
                } else {
                    None
                };

                controls(
                    ui,
                    &mut self.selected_provider,
                    &mut self.providers.keys(),
                    http_stats.as_ref(),
                    &mut self.images_plugin_data,
                );
                acknowledge(ui, attribution);
            }
        });
    }
}
