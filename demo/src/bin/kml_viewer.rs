#![cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]

use std::fs;
use std::path::PathBuf;

use eframe::App;
use egui::{self, Color32};
use rfd::FileDialog;
use walkers::{HttpOptions, HttpTiles, Map, MapMemory, Position, lon_lat};
use walkers_extras::{KmlFeature, KmlGeometry, KmlLayer, KmlVisualDefaults, parse_kml};

struct KmlViewerApp {
    memory: MapMemory,
    tiles: Option<HttpTiles>,
    layer: Option<KmlLayer>,
    file_name: Option<String>,
    load_error: Option<String>,
    summary: Option<String>,
}

impl Default for KmlViewerApp {
    fn default() -> Self {
        let mut memory = MapMemory::default();
        let _ = memory.set_zoom(5.0);
        memory.center_at(lon_lat(17.0, 52.0));

        let mut app = Self {
            memory,
            tiles: None,
            layer: None,
            file_name: None,
            load_error: None,
            summary: None,
        };

        // Auto-load Poland.kml if present (single canonical location),
        // or honor an explicit env var path if provided.
        let env_kml = std::env::var("KML_FILE")
            .or_else(|_| std::env::var("WALKERS_KML"))
            .ok()
            .and_then(|p| {
                let pb = std::path::PathBuf::from(p);
                if pb.exists() { Some(pb) } else { None }
            });
        let default_path = env_kml.or_else(|| {
            let p = std::path::PathBuf::from("demo/assets/Poland.kml");
            if p.exists() { Some(p) } else { None }
        });
        if let Some(path) = default_path {
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    if let Ok(features) = parse_kml(&content) {
                        let defaults = KmlVisualDefaults {
                            polygon_fill_color: Color32::from_rgba_unmultiplied(0, 0, 0, 0),
                            polygon_outline_color: Color32::from_rgb(0xFF, 0x00, 0x00),
                            polygon_outline_width: 3.0,
                            ..KmlVisualDefaults::default()
                        };
                        app.layer = Some(KmlLayer::new(features.clone()).with_defaults(defaults));
                        app.file_name = Some("Poland.kml".into());
                        app.summary = Some(summarize_features(&features));
                        if let Some(center) = centroid(&features) {
                            app.memory.center_at(center);
                        }
                        if let Some(mut zoom) = approximate_zoom(&features) {
                            // slight zoom out
                            zoom = (zoom - 0.5).max(1.0);
                            let _ = app.memory.set_zoom(zoom);
                        }
                    }
                }
                Err(err) => {
                    app.load_error = Some(format!("Failed to read Poland.kml: {err}"));
                }
            }
        }

        app
    }
}

impl KmlViewerApp {
    fn default_center() -> Position {
        lon_lat(17.0, 52.0)
    }

    fn ensure_tiles(&mut self, ctx: &egui::Context) {
        if self.tiles.is_none() {
            // Default sober basemap without token: Carto Light (Positron). Otherwise Mapbox Light if token.
            // Final fallback: OSM standard.
            let tiles = if let Ok(token) =
                std::env::var("MAPBOX_ACCESS_TOKEN").or_else(|_| std::env::var("MAPBOX_TOKEN"))
            {
                let src = walkers::sources::Mapbox {
                    style: walkers::sources::MapboxStyle::Light,
                    high_resolution: true,
                    access_token: token,
                };
                HttpTiles::with_options(src, HttpOptions::default(), ctx.clone())
            } else {
                // Local definition of a Carto Light XYZ tile source
                struct CartoLight;
                impl walkers::sources::TileSource for CartoLight {
                    fn tile_url(&self, tile_id: walkers::TileId) -> String {
                        // Use subdomain 'a' for simplicity
                        format!(
                            "https://cartodb-basemaps-a.global.ssl.fastly.net/light_all/{}/{}/{}.png",
                            tile_id.zoom, tile_id.x, tile_id.y
                        )
                    }
                    fn attribution(&self) -> walkers::sources::Attribution {
                        walkers::sources::Attribution {
                            text: "© OpenStreetMap © CARTO",
                            url: "https://carto.com/attributions",
                            logo_light: None,
                            logo_dark: None,
                        }
                    }
                    fn max_zoom(&self) -> u8 {
                        20
                    }
                }
                HttpTiles::with_options(CartoLight, HttpOptions::default(), ctx.clone())
            };
            self.tiles = Some(tiles);
        }
    }

    fn load_kml_from_path(&mut self, path: PathBuf) {
        match fs::read_to_string(&path) {
            Ok(content) => match parse_kml(&content) {
                Ok(features) => {
                    self.apply_features(path, features);
                }
                Err(err) => {
                    self.layer = None;
                    self.summary = None;
                    self.load_error = Some(format!("KML error: {err}"));
                }
            },
            Err(err) => {
                self.layer = None;
                self.summary = None;
                self.load_error = Some(format!("Failed to read file: {err}"));
            }
        }
    }

    fn apply_features(&mut self, path: PathBuf, features: Vec<KmlFeature>) {
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());

        let summary = summarize_features(&features);
        let center = centroid(&features);
        let zoom = approximate_zoom(&features);
        let defaults = KmlVisualDefaults::default();
        self.layer = Some(KmlLayer::new(features).with_defaults(defaults));
        self.file_name = Some(name);
        self.summary = Some(summary);
        self.load_error = None;

        if let Some(center) = center {
            self.memory.center_at(center);
        }
        if let Some(zoom) = zoom {
            let _ = self.memory.set_zoom(zoom);
        }
    }
}

impl App for KmlViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ensure_tiles(ctx);

        egui::TopBottomPanel::top("kml_controls").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open a KML file…").clicked() {
                    if let Some(path) = FileDialog::new().add_filter("KML", &["kml"]).pick_file() {
                        self.load_kml_from_path(path);
                    }
                }
                if let Some(name) = &self.file_name {
                    ui.separator();
                    ui.label(name);
                }
            });

            if let Some(summary) = &self.summary {
                ui.label(summary);
            }
            if let Some(error) = &self.load_error {
                ui.colored_label(Color32::RED, error);
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let center = self.memory.detached().unwrap_or_else(Self::default_center);
            let mut map = Map::new(None, &mut self.memory, center);

            if let Some(tiles) = self.tiles.as_mut() {
                map = map.with_layer(tiles, 1.0);
            }
            if let Some(layer) = self.layer.clone() {
                map = map.with_plugin(layer);
            }

            map.show(ui, |_, _, _| {});
        });
    }
}

fn summarize_features(features: &[KmlFeature]) -> String {
    let mut point_count = 0usize;
    let mut line_count = 0usize;
    let mut polygon_count = 0usize;

    for feature in features {
        for geometry in &feature.geometries {
            match geometry {
                KmlGeometry::Point(_) => point_count += 1,
                KmlGeometry::LineString(_) => line_count += 1,
                KmlGeometry::Polygon { .. } => polygon_count += 1,
            }
        }
    }

    format!(
        "Placemarks: {} | Points: {point_count} | Lines: {line_count} | Polygons: {polygon_count}",
        features.len()
    )
}

fn centroid(features: &[KmlFeature]) -> Option<Position> {
    let mut sum_lon = 0.0;
    let mut sum_lat = 0.0;
    let mut count = 0usize;

    for feature in features {
        for geometry in &feature.geometries {
            match geometry {
                KmlGeometry::Point(position) => {
                    sum_lon += position.x();
                    sum_lat += position.y();
                    count += 1;
                }
                KmlGeometry::LineString(points) => {
                    for point in points {
                        sum_lon += point.x();
                        sum_lat += point.y();
                        count += 1;
                    }
                }
                KmlGeometry::Polygon { exterior, holes } => {
                    for point in exterior {
                        sum_lon += point.x();
                        sum_lat += point.y();
                        count += 1;
                    }
                    for hole in holes {
                        for point in hole {
                            sum_lon += point.x();
                            sum_lat += point.y();
                            count += 1;
                        }
                    }
                }
            }
        }
    }

    if count == 0 {
        None
    } else {
        Some(Position::new(
            sum_lon / count as f64,
            sum_lat / count as f64,
        ))
    }
}

fn approximate_zoom(features: &[KmlFeature]) -> Option<f64> {
    let mut min_lon = f64::MAX;
    let mut max_lon = f64::MIN;
    let mut min_lat = f64::MAX;
    let mut max_lat = f64::MIN;
    let mut any = false;

    for feature in features {
        for geometry in &feature.geometries {
            let update = |lon: f64,
                          lat: f64,
                          min_lon: &mut f64,
                          max_lon: &mut f64,
                          min_lat: &mut f64,
                          max_lat: &mut f64| {
                *min_lon = (*min_lon).min(lon);
                *max_lon = (*max_lon).max(lon);
                *min_lat = (*min_lat).min(lat);
                *max_lat = (*max_lat).max(lat);
            };

            match geometry {
                KmlGeometry::Point(position) => {
                    update(
                        position.x(),
                        position.y(),
                        &mut min_lon,
                        &mut max_lon,
                        &mut min_lat,
                        &mut max_lat,
                    );
                    any = true;
                }
                KmlGeometry::LineString(points) => {
                    for point in points {
                        update(
                            point.x(),
                            point.y(),
                            &mut min_lon,
                            &mut max_lon,
                            &mut min_lat,
                            &mut max_lat,
                        );
                        any = true;
                    }
                }
                KmlGeometry::Polygon { exterior, holes } => {
                    for point in exterior {
                        update(
                            point.x(),
                            point.y(),
                            &mut min_lon,
                            &mut max_lon,
                            &mut min_lat,
                            &mut max_lat,
                        );
                        any = true;
                    }
                    for ring in holes {
                        for point in ring {
                            update(
                                point.x(),
                                point.y(),
                                &mut min_lon,
                                &mut max_lon,
                                &mut min_lat,
                                &mut max_lat,
                            );
                            any = true;
                        }
                    }
                }
            }
        }
    }

    if !any {
        return None;
    }

    let lon_span = (max_lon - min_lon).max(0.0001);
    let lat_span = (max_lat - min_lat).max(0.0001);
    let span = lon_span.max(lat_span);

    let zoom = if span > 60.0 {
        2.0
    } else if span > 30.0 {
        3.0
    } else if span > 10.0 {
        5.0
    } else if span > 5.0 {
        7.0
    } else if span > 2.0 {
        9.0
    } else if span > 1.0 {
        11.0
    } else if span > 0.5 {
        12.0
    } else if span > 0.2 {
        13.0
    } else if span > 0.05 {
        14.0
    } else {
        15.0
    };

    Some(zoom)
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Walkers: KML Viewer",
        options,
        Box::new(|_| Ok(Box::<KmlViewerApp>::default())),
    )
}
