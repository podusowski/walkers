use egui::Ui;
use geo::MapCoords;
use geo::geometry::Coord;
use geojson::{Feature, GeoJson};
use log::warn;
use walkers::{Context, Layer, Position, Projector, Style, render_line};

pub struct GeoJsonLayer {
    geojson: GeoJson,
    style: Style,
}

impl GeoJsonLayer {
    pub fn new(geojson: GeoJson, style: Style) -> Self {
        Self { geojson, style }
    }

    pub fn render(&self, ui: &mut Ui, projector: &Projector, zoom: u8) {
        let mut shapes = Vec::new();

        for layer in &self.style.layers {
            match layer {
                Layer::Line { paint, .. } => {
                    visit_features(&self.geojson, |feature| {
                        if let Some(geometry) = &feature.geometry {
                            let properties = feature
                                .properties
                                .clone()
                                .unwrap_or_default()
                                .into_iter()
                                .collect();

                            let geometry = walkers::Geometry::<f32>::try_from(geometry.clone())
                                .expect("invalid geometry");

                            let projected = project_geometry(&geometry, projector);

                            let _ = render_line(
                                &projected,
                                &Context::new("geometry_type/TODO".to_string(), properties, zoom),
                                &mut shapes,
                                paint,
                            );
                        }
                    });
                }
                other => {
                    warn!("Unsupported style layer: {other:?}");
                }
            }
        }

        let painter = ui.painter();
        for shape in shapes {
            match shape {
                walkers::ShapeOrText::Shape(shape) => {
                    painter.add(shape);
                }
                walkers::ShapeOrText::Text(_) => {
                    // Text rendering not yet supported for GeoJSON layers.
                }
            }
        }
    }
}

fn project_geometry(
    geometry: &walkers::Geometry<f32>,
    projector: &Projector,
) -> walkers::Geometry<f32> {
    geometry.map_coords(|coord| {
        let position = Position::new(coord.x as f64, coord.y as f64);
        let projected = projector.project(position);
        Coord {
            x: projected.x,
            y: projected.y,
        }
    })
}

fn visit_features(geojson: &GeoJson, mut visitor: impl FnMut(&Feature)) {
    match geojson {
        GeoJson::Geometry(_) => warn!("Top-level Geometry is not supported"),
        GeoJson::Feature(feature) => visitor(feature),
        GeoJson::FeatureCollection(feature_collection) => {
            for feature in &feature_collection.features {
                visitor(feature);
            }
        }
    }
}
