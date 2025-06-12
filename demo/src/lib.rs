mod local_tiles;
mod places;
mod plugins;
mod tiles;
mod windows;

use std::collections::BTreeMap;

use crate::plugins::ImagesPluginData;
use egui::{CentralPanel, Context, Frame};
use tiles::{providers, Provider, TilesKind};
use walkers::{Map, MapMemory};

pub struct MyApp {
    providers: BTreeMap<Provider, Vec<TilesKind>>,
    selected_provider: Provider,
    map_memory: MapMemory,
    images_plugin_data: ImagesPluginData,
    click_watcher: plugins::ClickWatcher,
    zoom_with_ctrl: bool,
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
            zoom_with_ctrl: true,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        CentralPanel::default().frame(Frame::NONE).show(ctx, |ui| {
            // Typically this would be a GPS acquired position which is tracked by the map.
            let my_position = places::wroclaw_glowny();

            let tiles = self.providers.get_mut(&self.selected_provider).unwrap();
            let attributions: Vec<_> = tiles
                .iter()
                .map(|tile| tile.as_ref().attribution())
                .collect();

            // In egui, widgets are constructed and consumed in each frame.
            let mut map = Map::new(None, &mut self.map_memory, my_position);

            // Various aspects of the map can be configured.
            map = map.zoom_with_ctrl(self.zoom_with_ctrl);

            // Optionally, plugins can be attached.
            map = map
                .with_plugin(plugins::places())
                .with_plugin(plugins::images(&mut self.images_plugin_data))
                .with_plugin(plugins::CustomShapes {})
                .with_plugin(&mut self.click_watcher);

            // Multiple layers can be added.
            for (n, tiles) in tiles.iter_mut().enumerate() {
                // With a different transparency.
                let transparency = if n == 0 { 1.0 } else { 0.25 };
                map = map.with_layer(tiles.as_mut(), transparency);
            }

            // Draw the map widget.
            ui.add(map);

            // Draw utility windows.
            {
                use windows::*;

                zoom(ui, &mut self.map_memory);
                go_to_my_position(ui, &mut self.map_memory);
                self.click_watcher.show_position(ui);

                let http_stats = tiles
                    .iter()
                    .filter_map(|tiles| {
                        if let TilesKind::Http(tiles) = tiles {
                            Some(tiles.stats())
                        } else {
                            None
                        }
                    })
                    .collect();

                controls(self, ui, http_stats);
                acknowledge(ui, attributions);
            }
        });
    }
}
