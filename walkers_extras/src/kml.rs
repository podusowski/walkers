use std::str::FromStr;
use std::sync::Arc;

use egui::{self, Color32, Response, Shape, Stroke, Ui};
use kml::{KmlDocument, types::Folder};
use log::debug;
use walkers::{MapMemory, Plugin, Projector, Style, lon_lat};

struct KmlLayerState {
    pub kml: kml::Kml,
    pub style: Style,
}

impl KmlLayerState {
    fn draw_line_layer(
        &self,
        painter: &egui::Painter,
        response: &Response,
        projector: &Projector,
        element: &kml::Kml,
    ) {
        match element {
            kml::Kml::Placemark(placemark) => {
                if let Some(geometry) = &placemark.geometry {
                    self.draw_line_geometry(&painter, response, projector, geometry);
                }
            }
            kml::Kml::Document { elements, .. }
            | kml::Kml::KmlDocument(KmlDocument { elements, .. })
            | kml::Kml::Folder(Folder { elements, .. }) => {
                for child in elements {
                    self.draw_line_layer(painter, response, projector, child);
                }
            }
            _ => {
                debug!("Skipping unsupported KML element: {:?}", element);
            }
        }
    }

    fn draw_line_geometry(
        &self,
        painter: &egui::Painter,
        response: &Response,
        projector: &Projector,
        geometry: &kml::types::Geometry,
    ) {
        match geometry {
            kml::types::Geometry::Polygon(polygon) => {
                let line_width = 2.0;
                let stroke = Stroke::new(line_width, Color32::BLACK);

                let exterior: Vec<_> = polygon
                    .outer
                    .coords
                    .iter()
                    .map(|c| projector.project(lon_lat(c.x, c.y)).to_pos2())
                    .collect();

                painter.add(Shape::closed_line(exterior, stroke));

                for inner in &polygon.inner {
                    let hole: Vec<_> = inner
                        .coords
                        .iter()
                        .map(|c| projector.project(lon_lat(c.x, c.y)).to_pos2())
                        .collect();

                    painter.add(Shape::closed_line(hole, stroke));
                }
            }
            kml::types::Geometry::MultiGeometry(multi_geometry) => {
                for geom in &multi_geometry.geometries {
                    self.draw_line_geometry(painter, response, projector, geom);
                }
            }
            _ => todo!(),
        }
    }
}

/// Plugin that renders parsed KML features on top of a [`Map`](walkers::Map).
#[derive(Clone)]
pub struct KmlLayer {
    inner: Arc<KmlLayerState>,
}

impl KmlLayer {
    pub fn from_string(s: &str, style: Style) -> Self {
        Self {
            inner: Arc::new(KmlLayerState {
                kml: kml::Kml::from_str(s).unwrap(),
                style,
            }),
        }
    }
}

impl Plugin for KmlLayer {
    fn run(
        self: Box<Self>,
        ui: &mut Ui,
        response: &Response,
        projector: &Projector,
        _map_memory: &MapMemory,
    ) {
        for layer in &self.inner.style.layers {
            match layer {
                walkers::Layer::Line { .. } => {
                    self.inner.draw_line_layer(
                        &ui.painter_at(response.rect),
                        response,
                        projector,
                        &self.inner.kml,
                    );
                }
                other => {
                    log::warn!("Unsupported KML Layer style layer: {:?}", other);
                }
            }
        }
    }
}
