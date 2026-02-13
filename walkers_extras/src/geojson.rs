use egui::{Response, Ui};
use log::warn;
use walkers::{Layer, MapMemory, Plugin, Projector, Style};

pub struct GeoJsonLayer {
    geojson: geojson::GeoJson,
    style: Style,
}

impl GeoJsonLayer {
    pub fn new(geojson: geojson::GeoJson, style: Style) -> Self {
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
                    todo!();
                }
                other => {
                    warn!("Unsupported style layer: {other:?}");
                }
            }
        }
    }
}
