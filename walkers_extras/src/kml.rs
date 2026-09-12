use std::str::FromStr;

use egui::{self, Color32, Response, Shape, Stroke, Ui};
use kml::{KmlDocument, types::Folder};
use log::{debug, warn};
use walkers::{Layer, Plugin, Projection, ScreenProjector, Style, lon_lat};

/// Plugin that renders parsed KML features on top of a [`Map`](walkers::Map).
pub struct KmlLayer {
    kml: kml::Kml,
    style: Style,
}

impl KmlLayer {
    pub fn from_string(s: &str, style: Style) -> Self {
        Self {
            kml: kml::Kml::from_str(s).unwrap(),
            style,
        }
    }
}

fn draw_line_layer<P: Projection + ?Sized>(
    painter: &egui::Painter,
    projector: &ScreenProjector<'_, P>,
    element: &kml::Kml,
) {
    match element {
        kml::Kml::Placemark(placemark) => {
            if let Some(geometry) = &placemark.geometry {
                draw_line_geometry(painter, projector, geometry);
            }
        }
        kml::Kml::Document { elements, .. }
        | kml::Kml::KmlDocument(KmlDocument { elements, .. })
        | kml::Kml::Folder(Folder { elements, .. }) => {
            for child in elements {
                draw_line_layer(painter, projector, child);
            }
        }
        _ => {
            debug!("Skipping unsupported KML element: {element:?}");
        }
    }
}

fn draw_circle_layer<P: Projection + ?Sized>(
    painter: &egui::Painter,
    projector: &ScreenProjector<'_, P>,
    element: &kml::Kml,
) {
    match element {
        kml::Kml::Placemark(placemark) => {
            if let Some(geometry) = &placemark.geometry {
                draw_circle_geometry(painter, projector, geometry);
            }
        }
        kml::Kml::Document { elements, .. }
        | kml::Kml::KmlDocument(KmlDocument { elements, .. })
        | kml::Kml::Folder(Folder { elements, .. }) => {
            for child in elements {
                draw_circle_layer(painter, projector, child);
            }
        }
        _ => {
            debug!("Skipping unsupported KML element: {element:?}");
        }
    }
}

fn draw_line_geometry<P: Projection + ?Sized>(
    painter: &egui::Painter,
    projector: &ScreenProjector<'_, P>,
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
                .map(|c| projector.project(lon_lat(c.x, c.y)))
                .collect();

            painter.add(Shape::closed_line(exterior, stroke));

            for inner in &polygon.inner {
                let hole: Vec<_> = inner
                    .coords
                    .iter()
                    .map(|c| projector.project(lon_lat(c.x, c.y)))
                    .collect();

                painter.add(Shape::closed_line(hole, stroke));
            }
        }
        kml::types::Geometry::MultiGeometry(multi_geometry) => {
            for geom in &multi_geometry.geometries {
                draw_line_geometry(painter, projector, geom);
            }
        }
        _ => todo!(),
    }
}

fn draw_circle_geometry<P: Projection + ?Sized>(
    painter: &egui::Painter,
    projector: &ScreenProjector<'_, P>,
    geometry: &kml::types::Geometry,
) {
    match geometry {
        kml::types::Geometry::Point(point) => {
            let center = projector.project(lon_lat(point.coord.x, point.coord.y));
            let radius = 5.0;
            let stroke = Stroke::new(1.0, Color32::BLACK);
            let fill = Color32::from_rgb(0, 255, 0);

            painter.add(Shape::circle_filled(center, radius, fill));
            painter.add(Shape::circle_stroke(center, radius, stroke));
        }
        kml::types::Geometry::MultiGeometry(multi_geometry) => {
            for geom in &multi_geometry.geometries {
                draw_circle_geometry(painter, projector, geom);
            }
        }
        _ => todo!(),
    }
}

impl<P: Projection> Plugin<P> for KmlLayer {
    fn run(self: Box<Self>, ui: &mut Ui, response: &Response, projector: &ScreenProjector<'_, P>) {
        for layer in &self.style.layers {
            match layer {
                Layer::Line { .. } => {
                    draw_line_layer(&ui.painter_at(response.rect), projector, &self.kml);
                }
                Layer::Circle { .. } => {
                    draw_circle_layer(&ui.painter_at(response.rect), projector, &self.kml);
                }
                other => {
                    warn!("Unsupported style layer: {other:?}");
                }
            }
        }
    }
}
