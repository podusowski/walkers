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
    providers: BTreeMap<Provider, TilesKind>,
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

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        CentralPanel::default().frame(Frame::NONE).show(ctx, |ui| {
            // Typically this would be a GPS acquired position which is tracked by the map.
            let my_position = places::wroclaw_glowny();

            let tiles = self.providers.get_mut(&self.selected_provider).unwrap();
            let attribution = tiles.as_ref().attribution();

            // In egui, widgets are constructed and consumed in each frame.
            let map = Map::new(Some(tiles.as_mut()), &mut self.map_memory, my_position);

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

                let http_stats = if let TilesKind::Http(tiles) = tiles {
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
