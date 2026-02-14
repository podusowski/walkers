use egui::{Response, Ui};
use geojson::{Feature, GeoJson};
use log::warn;
use walkers::{Layer, MapMemory, Plugin, Projector, Style};

pub struct GeoJsonLayer {
    geojson: GeoJson,
    style: Style,
}

impl GeoJsonLayer {
    pub fn new(geojson: GeoJson, style: Style) -> Self {
        Self { geojson, style }
    }
}

impl Plugin for GeoJsonLayer {
    fn run(
        self: Box<Self>,
        ui: &mut Ui,
        response: &Response,
        projector: &Projector,
        _map_memory: &MapMemory,
    ) {
        for layer in &self.style.layers {
            match layer {
                Layer::Line { .. } => {
                    visit_features(&self.geojson, |feature| {
                        if let Some(geometry) = &feature.geometry {
                            //render_line(geometry, Conte
                            todo!();
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

fn visit_features(geojson: &GeoJson, visitor: impl Fn(&Feature)) {
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
