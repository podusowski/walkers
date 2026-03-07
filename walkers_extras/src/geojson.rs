use egui::{Response, Ui};
use geojson::{Feature, GeoJson};
use log::warn;
use walkers::{Context, Layer, MapMemory, Plugin, Projector, Style, render_line};

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
                            render_line(
                                &walkers::Geometry::<f32>::try_from(geometry.clone())
                                    .expect("invalid geometry"),
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
    }
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
