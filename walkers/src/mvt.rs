//! Renderer for Mapbox Vector Tiles.

use egui::{
    epaint::{PathShape, PathStroke},
    pos2, Color32, Pos2,
};
use geo_types::Geometry;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Mvt(#[from] mvt_reader::error::ParserError),
    #[error("Mvt rendering error: {0}")]
    Other(String),
}

pub fn render(
    tile: &mvt_reader::Reader,
    painter: egui::Painter,
    rect: egui::Rect,
) -> Result<(), Error> {
    #[cfg(feature = "debug_mvt_rendering")]
    // Draw a rect around the tile.
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0, Color32::RED),
        egui::StrokeKind::Inside,
    );

    // Tile coords are from 0 to 4096, but we have a rect to fill.
    let transformed_pos2 = |x: f32, y: f32| {
        pos2(
            rect.left() + (x / 4096.0) * rect.width(),
            rect.top() + (y / 4096.0) * rect.height(),
        )
    };

    // That is just dumb, but mvt-reader API sucks.
    for (i, metadata) in tile.get_layer_metadata().unwrap().iter().enumerate() {
        if metadata.extent != 4096 {
            return Err(Error::Other(format!(
                "Unsupported extent: {}, expected 4096",
                metadata.extent
            )));
        }

        for feature in tile.get_features(i)? {
            match feature.geometry {
                Geometry::Point(_point) => todo!(),
                Geometry::Line(_line) => todo!(),
                Geometry::LineString(line_string) => {
                    for segment in line_string.0.windows(2) {
                        painter.line_segment(
                            [
                                transformed_pos2(segment[0].x, segment[0].y),
                                transformed_pos2(segment[1].x, segment[1].y),
                            ],
                            egui::Stroke::new(1.0, Color32::from_rgb(200, 200, 200)),
                        );
                    }
                }
                Geometry::Polygon(_polygon) => todo!(),
                Geometry::MultiPoint(multi_point) => {
                    for point in multi_point {
                        painter.circle_filled(
                            transformed_pos2(point.x(), point.y()),
                            3.0,
                            Color32::from_rgb(200, 200, 0),
                        );
                    }
                }
                Geometry::MultiLineString(multi_line_string) => {
                    for line_string in multi_line_string {
                        let points = line_string
                            .0
                            .iter()
                            .map(|p| transformed_pos2(p.x, p.y))
                            .collect::<Vec<_>>();
                        let stroke = egui::Stroke::new(2.0, Color32::ORANGE);
                        painter.line(points, stroke);
                    }
                }
                Geometry::MultiPolygon(multi_polygon) => {
                    for polygon in multi_polygon.iter() {
                        let points = polygon
                            .exterior()
                            .0
                            .iter()
                            .map(|p| transformed_pos2(p.x, p.y))
                            .collect::<Vec<_>>();
                        arbitrary_polygon(&points, &painter);
                    }
                }
                Geometry::GeometryCollection(_geometry_collection) => todo!(),
                Geometry::Rect(_rect) => todo!(),
                Geometry::Triangle(_triangle) => todo!(),
            }
        }
    }
    Ok(())
}

/// Egui can only draw convex polygons, so we need to triangulate arbitrary ones.
fn arbitrary_polygon(points: &[Pos2], painter: &egui::Painter) {
    let mut triangles = Vec::<usize>::new();
    let mut earcut = earcut::Earcut::new();
    earcut.earcut(points.iter().map(|p| [p.x, p.y]), &[], &mut triangles);

    for triangle_indices in triangles.chunks(3) {
        let triangle = [
            points[triangle_indices[0]],
            points[triangle_indices[1]],
            points[triangle_indices[2]],
        ];

        if triangle_area(triangle[0], triangle[1], triangle[2]) < 0.1 {
            // Too small to render without artifacts.
            continue;
        }

        painter.add(PathShape::convex_polygon(
            triangle.to_vec(),
            Color32::from_rgb(100, 150, 200).gamma_multiply(0.5),
            PathStroke::NONE,
        ));

        #[cfg(feature = "debug_mvt_rendering")]
        painter.add(PathShape::closed_line(
            triangle.to_vec(),
            PathStroke::new(2.0, Color32::RED),
        ));
    }
}

fn triangle_area(a: Pos2, b: Pos2, c: Pos2) -> f32 {
    ((a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y)) / 2.0).abs()
}
