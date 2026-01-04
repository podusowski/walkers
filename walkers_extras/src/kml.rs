use std::str::FromStr;
use std::sync::Arc;

use egui::{self, Color32, Response, Shape, Stroke, Ui};
use kml::KmlDocument;
use lyon_path::geom::Point;
use lyon_tessellation::math::point;
use walkers::{MapMemory, Plugin, Position, Projector, Style, lon_lat, tessellate_polygon};

struct KmlLayerState {
    pub kml: kml::Kml,
    pub style: Style,
}

impl KmlLayerState {
    fn draw_geometry(
        &self,
        painter: &egui::Painter,
        response: &Response,
        projector: &Projector,
        geometry: &kml::types::Geometry,
    ) {
        println!("Drawing geometry: {:?}", geometry);
        match geometry {
            kml::types::Geometry::Point(point) => {
                let position = lon_lat(point.coord.x, point.coord.y);
                // TODO: Take this from style.
                let radius = 5.0;
                let color = Color32::RED;
                let screen = projector.project(position).to_pos2();
                painter.circle_filled(screen, radius, color);
            }
            kml::types::Geometry::LineString(_) => todo!(),
            kml::types::Geometry::LinearRing(_) => todo!(),
            kml::types::Geometry::Polygon(polygon) => {
                let exterior = &polygon.outer.coords;
                let holes: Vec<&Vec<kml::types::Coord>> =
                    polygon.inner.iter().map(|b| &b.coords).collect();
                let exterior_positions: Vec<Position> =
                    exterior.iter().map(|c| lon_lat(c.x, c.y)).collect();
                let mut holes_positions: Vec<Vec<Position>> = Vec::new();
                for hole in holes {
                    let hole_positions: Vec<Position> =
                        hole.iter().map(|c| lon_lat(c.x, c.y)).collect();
                    holes_positions.push(hole_positions);
                }
                draw_polygon(painter, projector, &exterior_positions, &holes_positions);
            }
            kml::types::Geometry::MultiGeometry(multi_geometry) => {
                for geom in &multi_geometry.geometries {
                    self.draw_geometry(painter, response, projector, geom);
                }
            }
            _ => todo!(),
        }
    }

    fn draw(&self, ui: &mut Ui, response: &Response, projector: &Projector, element: &kml::Kml) {
        let painter = ui.painter_at(response.rect);

        match element {
            kml::Kml::Placemark(placemark) => {
                println!("Drawing placemark: {:?}", placemark);
                if let Some(geometry) = &placemark.geometry {
                    self.draw_geometry(&painter, response, projector, geometry);
                }
            }
            kml::Kml::Document { elements, .. } => {
                println!("Drawing document with {} elements", elements.len());
                for child in elements {
                    self.draw(ui, response, projector, child);
                }
            }
            kml::Kml::KmlDocument(KmlDocument { elements, .. }) => {
                println!("Drawing kml document with {} elements", elements.len());
                for child in elements {
                    self.draw(ui, response, projector, child);
                }
            }
            kml::Kml::Folder(folder) => {
                println!("Drawing folder with {} elements", folder.elements.len());
                for child in &folder.elements {
                    self.draw(ui, response, projector, child);
                }
            }
            _ => {
                println!("Skipping unsupported KML element: {:?}", element);
            }
        }
    }

    fn draw_line_layer(
        &self,
        painter: &egui::Painter,
        response: &Response,
        projector: &Projector,
        element: &kml::Kml,
    ) {
        match element {
            kml::Kml::Placemark(placemark) => {
                println!("Drawing placemark: {:?}", placemark);
                if let Some(geometry) = &placemark.geometry {
                    self.draw_line_geometry(&painter, response, projector, geometry);
                }
            }
            kml::Kml::Document { elements, .. } => {
                println!("Drawing document with {} elements", elements.len());
                for child in elements {
                    self.draw_line_layer(painter, response, projector, child);
                }
            }
            kml::Kml::KmlDocument(KmlDocument { elements, .. }) => {
                println!("Drawing kml document with {} elements", elements.len());
                for child in elements {
                    self.draw_line_layer(painter, response, projector, child);
                }
            }
            kml::Kml::Folder(folder) => {
                println!("Drawing folder with {} elements", folder.elements.len());
                for child in &folder.elements {
                    self.draw_line_layer(painter, response, projector, child);
                }
            }
            _ => {
                println!("Skipping unsupported KML element: {:?}", element);
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
            kml::types::Geometry::LineString(_) => todo!(),
            kml::types::Geometry::LinearRing(_) => todo!(),
            kml::types::Geometry::Polygon(polygon) => {
                let exterior = &polygon.outer.coords;
                let holes: Vec<&Vec<kml::types::Coord>> =
                    polygon.inner.iter().map(|b| &b.coords).collect();
                let exterior_positions: Vec<Position> =
                    exterior.iter().map(|c| lon_lat(c.x, c.y)).collect();

                let mut holes_positions: Vec<Vec<Position>> = Vec::new();
                for hole in &holes {
                    let hole_positions: Vec<Position> =
                        hole.iter().map(|c| lon_lat(c.x, c.y)).collect();
                    holes_positions.push(hole_positions);
                }


                let Some(exterior_screen) = ring_to_screen_points(&exterior_positions, projector) else {
                    return;
                };

                let mut hole_points: Vec<Vec<Point<f32>>> = Vec::with_capacity(holes.len());
                for hole in holes_positions {
                    if let Some(points) = ring_to_screen_points(&hole, projector) {
                        hole_points.push(points);
                    }
                }


                let line_width = 2.0;
                let stroke = Stroke::new(line_width, Color32::BLACK);

                painter.add(Shape::closed_line(
                    exterior_screen
                        .iter()
                        .map(|p| egui::pos2(p.x, p.y))
                        .collect(),
                    stroke,
                ));
                for hole in &hole_points {
                    painter.add(Shape::closed_line(
                        hole.iter().map(|p| egui::pos2(p.x, p.y)).collect(),
                        stroke,
                    ));
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
            println!("KML Layer style layer: {:?}", layer);
            match layer {
                walkers::Layer::Background { paint } => todo!(),
                walkers::Layer::Fill {
                    source_layer,
                    filter,
                    paint,
                } => todo!(),
                walkers::Layer::Line {
                    source_layer,
                    filter,
                    paint,
                } => {
                    self.inner.draw_line_layer(
                        &ui.painter_at(response.rect),
                        response,
                        projector,
                        &self.inner.kml,
                    );
                }
                walkers::Layer::Symbol {
                    source_layer,
                    filter,
                    layout,
                    paint,
                } => todo!(),
                other => {
                    log::warn!("Unsupported KML Layer style layer: {:?}", other);
                }
            }
            self.inner.draw(ui, response, projector, &self.inner.kml);
        }
    }
}

fn draw_polygon(
    painter: &egui::Painter,
    projector: &Projector,
    exterior: &[Position],
    holes: &[Vec<Position>],
) {
    let Some(exterior_screen) = ring_to_screen_points(exterior, projector) else {
        return;
    };

    let mut hole_points: Vec<Vec<Point<f32>>> = Vec::with_capacity(holes.len());
    for hole in holes {
        if let Some(points) = ring_to_screen_points(hole, projector) {
            hole_points.push(points);
        }
    }

    // TODO: Support this.
    let fill_color = None;

    if let Some(fill_color) = fill_color {
        if let Ok(mesh) = tessellate_polygon(&exterior_screen, &hole_points, fill_color) {
            painter.add(Shape::mesh(mesh));
        }
    }

    let line_width = 2.0;
    let stroke = Stroke::new(line_width, Color32::BLACK);

    painter.add(Shape::closed_line(
        exterior_screen
            .iter()
            .map(|p| egui::pos2(p.x, p.y))
            .collect(),
        stroke,
    ));
    for hole in &hole_points {
        painter.add(Shape::closed_line(
            hole.iter().map(|p| egui::pos2(p.x, p.y)).collect(),
            stroke,
        ));
    }
}

fn ring_to_screen_points(ring: &[Position], projector: &Projector) -> Option<Vec<Point<f32>>> {
    if ring.len() < 3 {
        return None;
    }
    let mut points = Vec::with_capacity(ring.len());
    for (idx, position) in ring.iter().enumerate() {
        if idx + 1 == ring.len() && ring[0].x() == position.x() && ring[0].y() == position.y() {
            // Skip duplicate closing vertex.
            continue;
        }
        let p = projector.project(*position).to_pos2();
        points.push(point(p.x, p.y));
    }
    if points.len() < 3 { None } else { Some(points) }
}
