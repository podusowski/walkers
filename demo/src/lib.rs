mod kml;
mod places;
mod plugins;
mod tiles;
mod windows;

use egui::{Button, Context, DragPanButtons, OpenUrl, Rect, Vec2};
use tiles::{TilesKind, providers};
use walkers::{Color, Filter, Float, Layer, Map, MapMemory, Paint, Style, json};
use walkers_extras::GeoJsonLayer;

use crate::tiles::Providers;

pub struct MyApp {
    providers: Providers,
    map_memory: MapMemory,
    click_watcher: plugins::ClickWatcher,
    zoom_with_ctrl: bool,
    geojson_layers: Vec<GeoJsonLayer>,
}

impl MyApp {
    pub fn new(egui_ctx: Context) -> Self {
        egui_extras::install_image_loaders(&egui_ctx);

        Self {
            providers: providers(egui_ctx.to_owned()),
            map_memory: MapMemory::default(),
            click_watcher: Default::default(),
            zoom_with_ctrl: true,
            geojson_layers: geojson_layers(),
        }
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Typically this would be a GPS acquired position which is tracked by the map.
        let my_position = places::wroclaw_glowny();

        let tiles = self
            .providers
            .available
            .get_mut(&self.providers.selected)
            .unwrap();
        let attributions: Vec<_> = tiles
            .iter()
            .map(|tile| tile.as_ref().attribution())
            .collect();

        // In egui, widgets are constructed and consumed in each frame.
        let mut map = Map::new(None, &mut self.map_memory, my_position);

        // Various aspects of the map can be configured.
        map = map
            .zoom_with_ctrl(self.zoom_with_ctrl)
            .drag_pan_buttons(DragPanButtons::PRIMARY | DragPanButtons::SECONDARY);

        // Optionally, plugins can be attached.
        map = map
            .with_plugin(plugins::places())
            .with_plugin(plugins::CustomShapes {})
            .with_plugin(&mut self.click_watcher)
            .with_plugin(kml::poland_borders())
            .with_plugin(kml::outgym_umea_layer());

        // Multiple layers can be added.
        for (n, tiles) in tiles.iter_mut().enumerate() {
            // With a different transparency.
            let transparency = if n == 0 { 1.0 } else { 0.25 };
            map = map.with_layer(tiles.as_mut(), transparency);
        }

        // Draw the map widget.
        let response = map.show(ui, |ui, _, projector, map_memory| {
            for layer in &self.geojson_layers {
                layer.render(ui, projector, map_memory.zoom().round() as u8);
            }

            // You can add any additional contents to the map's UI here.
            let bastion = projector.project(places::bastion_sakwowy()).to_pos2();
            ui.put(
                Rect::from_center_size(bastion, Vec2::new(140., 20.)),
                Button::new("Bastion Sakwowy"),
            )
            .on_hover_text("Click to see some information about this place.")
            .clicked()
            .then_some("https://www.wroclaw.pl/dla-mieszkanca/bastion-sakwowy-wroclaw-atrakcje")
        });

        // Could have done it in the closure, but this way you can see how to pass values outside.
        if let Some(url) = response.inner {
            ui.open_url(OpenUrl::new_tab(url));
        }

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

            controls(self, ui, http_stats, _frame);
            acknowledge(ui, attributions);
        }
    }
}

/// Find `.geojson` files in the current directory and build GeoJsonLayer out of them.
fn geojson_layers() -> Vec<GeoJsonLayer> {
    use std::fs;

    let mut layers = Vec::new();

    for entry in fs::read_dir(".").unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("geojson") {
            let content = fs::read_to_string(path).unwrap();
            let geojson = content.parse().unwrap();
            layers.push(GeoJsonLayer::new(geojson, trails_style()));
        }
    }

    layers
}

fn trails_style() -> Style {
    let width = |factor| {
        Float(json!([
            "interpolate",
            ["linear"],
            ["zoom"],
            10.0,
            0.1 * factor,
            26.0,
            10.0 * factor
        ]))
    };

    Style {
        layers: vec![
            Layer::Line {
                source_layer: "".to_string(),
                filter: Some(Filter(json!([
                    "any",
                    ["==", ["get", "colour"], "red"],
                    ["==", ["get", "colour"], "blue"],
                    ["==", ["get", "colour"], "black"],
                ]))),
                paint: Paint {
                    line_color: Some(Color(json!("#cdcdcd"))),
                    line_width: Some(width(1.0)),
                    ..Default::default()
                },
            },
            Layer::Line {
                source_layer: "".to_string(),
                filter: Some(Filter(json!([
                    "any",
                    ["==", ["get", "colour"], "yellow"],
                    ["==", ["get", "colour"], "green"]
                ]))),
                paint: Paint {
                    line_color: Some(Color(json!("#000000"))),
                    line_width: Some(width(1.0)),
                    ..Default::default()
                },
            },
            Layer::Line {
                source_layer: "".to_string(),
                filter: Some(Filter(json!(["==", ["get", "colour"], "red"]))),
                paint: Paint {
                    line_color: Some(Color(json!("#7b0000"))),
                    line_width: Some(width(0.8)),
                    ..Default::default()
                },
            },
            Layer::Line {
                source_layer: "".to_string(),
                filter: Some(Filter(json!(["==", ["get", "colour"], "blue"]))),
                paint: Paint {
                    line_color: Some(Color(json!("#0028ac"))),
                    line_width: Some(width(0.6)),
                    ..Default::default()
                },
            },
            Layer::Line {
                source_layer: "".to_string(),
                filter: Some(Filter(json!(["==", ["get", "colour"], "green"]))),
                paint: Paint {
                    line_color: Some(Color(json!("#005d09"))),
                    line_width: Some(width(0.4)),
                    ..Default::default()
                },
            },
            Layer::Line {
                source_layer: "".to_string(),
                filter: Some(Filter(json!(["==", ["get", "colour"], "yellow"]))),
                paint: Paint {
                    line_color: Some(Color(json!("#bbbb00"))),
                    line_width: Some(width(0.3)),
                    ..Default::default()
                },
            },
            Layer::Line {
                source_layer: "".to_string(),
                filter: Some(Filter(json!(["==", ["get", "colour"], "black"]))),
                paint: Paint {
                    line_color: Some(Color(json!("#000000"))),
                    line_width: Some(width(0.2)),
                    ..Default::default()
                },
            },
        ],
    }
}
