use std::{collections::HashMap, f32::consts::TAU, str::FromStr};

use egui::{Color32, Pos2, Vec2};
use geo::MapCoords;
use geo::geometry::Coord;
use log::warn;
use walkers::geo_types::{Geometry, Point};
use walkers::{
    Canvas, Context, Drawable, Layer, Line, Mesh, Paint, Position, Projector, Style, Vertex,
    render_fill, render_line,
};

/// Parsed KML features, drawn on top of a [`Map`](walkers::Map).
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

    pub fn render(&self, canvas: &mut Canvas, projector: &Projector, zoom: u8) {
        for layer in &self.style.layers {
            match layer {
                Layer::Fill { paint, .. } => {
                    self.draw_fills(canvas, projector, paint, zoom);
                }
                Layer::Line { paint, .. } => {
                    self.draw_lines(canvas, projector, paint, zoom);
                }
                Layer::Circle { .. } => {
                    self.draw_circles(canvas, projector);
                }
                other => {
                    warn!("Unsupported style layer: {other:?}");
                }
            }
        }
    }

    fn draw_lines(&self, canvas: &mut Canvas, projector: &Projector, paint: &Paint, zoom: u8) {
        let context = Context::new("LineString".to_string(), HashMap::new(), zoom);

        for geometry in &self.geometries {
            let projected = project(geometry, projector);
            let _ = render_line(&projected, &context, paint, &mut canvas.drawables);
        }
    }

    fn draw_fills(&self, canvas: &mut Canvas, projector: &Projector, paint: &Paint, zoom: u8) {
        let context = Context::new("Polygon".to_string(), HashMap::new(), zoom);

        for geometry in &self.geometries {
            let projected = project(geometry, projector);
            if let Err(err) = render_fill(&projected, &context, paint, &mut canvas.drawables) {
                warn!("{err}");
            }
        }
    }

    fn draw_circles(&self, canvas: &mut Canvas, projector: &Projector) {
        for point in self.geometries.iter().flat_map(points) {
            let center = projector.project(*point).to_pos2();
            canvas.drawables.extend(circle(
                center,
                5.0,
                Color32::from_rgb(0, 255, 0),
                Color32::BLACK,
            ));
        }
    }
}

/// Filled circle with a 1px outline.
fn circle(center: Pos2, radius: f32, fill: Color32, outline: Color32) -> [Drawable; 2] {
    const SEGMENTS: u32 = 16;

    let rim: Vec<Pos2> = (0..SEGMENTS)
        .map(|i| center + radius * Vec2::angled(i as f32 * TAU / SEGMENTS as f32))
        .collect();

    let vertices = std::iter::once(center)
        .chain(rim.iter().copied())
        .map(|position| Vertex {
            position,
            color: fill,
        })
        .collect();

    // A fan around the center, which is the first vertex.
    let indices = (0..SEGMENTS)
        .flat_map(|i| [0, 1 + i, 1 + (i + 1) % SEGMENTS])
        .collect();

    let mut points = rim.clone();
    points.push(rim[0]);

    [
        Drawable::Fill(Mesh { vertices, indices }),
        Drawable::Lines(vec![Line {
            points,
            width: 1.0,
            color: outline,
            dasharray: Vec::new(),
        }]),
    ]
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
