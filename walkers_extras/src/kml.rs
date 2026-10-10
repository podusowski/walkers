use std::{collections::HashMap, str::FromStr};

use egui::{self, Color32, Response, Shape, Stroke, Ui};
use geo::MapCoords;
use geo::geometry::Coord;
use log::warn;
use walkers::geo_types::{Geometry, Point};
use walkers::{
    Context, Layer, MapMemory, Paint, Plugin, Position, Projector, Style, render_fill, render_line,
    to_shapes,
};

/// Plugin that renders parsed KML features on top of a [`Map`](walkers::Map).
pub struct KmlLayer {
    geometries: Vec<Geometry<f64>>,
    style: Style,
}

impl KmlLayer {
    pub fn from_string(s: &str, style: Style) -> Self {
        let kml = kml::Kml::<f64>::from_str(s).unwrap();
        Self {
            geometries: Vec::try_from(kml).unwrap_or_default(),
            style,
        }
    }

    fn draw_lines(&self, painter: &egui::Painter, projector: &Projector, paint: &Paint, zoom: u8) {
        let context = Context::new("LineString".to_string(), HashMap::new(), zoom);
        let mut drawables = Vec::new();

        for geometry in &self.geometries {
            let projected = project(geometry, projector);
            let _ = render_line(&projected, &context, paint, &mut drawables);
        }

        painter.extend(to_shapes(&drawables));
    }

    fn draw_fills(&self, painter: &egui::Painter, projector: &Projector, paint: &Paint, zoom: u8) {
        let context = Context::new("Polygon".to_string(), HashMap::new(), zoom);
        let mut drawables = Vec::new();

        for geometry in &self.geometries {
            let projected = project(geometry, projector);
            if let Err(err) = render_fill(&projected, &context, paint, &mut drawables) {
                warn!("{err}");
            }
        }

        painter.extend(to_shapes(&drawables));
    }

    fn draw_circles(&self, painter: &egui::Painter, projector: &Projector) {
        for point in self.geometries.iter().flat_map(points) {
            let center = projector.project(*point).to_pos2();
            let radius = 5.0;
            let stroke = Stroke::new(1.0, Color32::BLACK);
            let fill = Color32::from_rgb(0, 255, 0);

            painter.add(Shape::circle_filled(center, radius, fill));
            painter.add(Shape::circle_stroke(center, radius, stroke));
        }
    }
}

fn points(geometry: &Geometry<f64>) -> Vec<&Point<f64>> {
    match geometry {
        Geometry::Point(point) => vec![point],
        Geometry::MultiPoint(multi_point) => multi_point.iter().collect(),
        Geometry::GeometryCollection(collection) => collection.iter().flat_map(points).collect(),
        _ => Vec::new(),
    }
}

/// From longitude and latitude onto the screen.
fn project(geometry: &Geometry<f64>, projector: &Projector) -> Geometry<f32> {
    geometry.map_coords(|coord| {
        let projected = projector.project(Position::new(coord.x, coord.y));
        Coord {
            x: projected.x,
            y: projected.y,
        }
    })
}

impl Plugin for KmlLayer {
    fn run(
        self: Box<Self>,
        ui: &mut Ui,
        response: &Response,
        projector: &Projector,
        map_memory: &MapMemory,
    ) {
        let painter = ui.painter_at(response.rect);
        let zoom = map_memory.zoom().round() as u8;

        for layer in &self.style.layers {
            match layer {
                Layer::Fill { paint, .. } => {
                    self.draw_fills(&painter, projector, paint, zoom);
                }
                Layer::Line { paint, .. } => {
                    self.draw_lines(&painter, projector, paint, zoom);
                }
                Layer::Circle { .. } => {
                    self.draw_circles(&painter, projector);
                }
                other => {
                    warn!("Unsupported style layer: {other:?}");
                }
            }
        }
    }
}
