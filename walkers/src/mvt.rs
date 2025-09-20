//! Renderer for Mapbox Vector Tiles.

use egui::{pos2, Color32, Shape};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Mvt(#[from] mvt_reader::error::ParserError),
    #[error("Mvt rendering error: {0}")]
    Other(String),
}

pub fn render2(
    tile: &mvt_reader::Reader,
    painter: &egui::Painter,
    rect: egui::Rect,
) -> Result<(), Error> {
    // debug box around the tile
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
                geo_types::Geometry::Point(_point) => todo!(),
                geo_types::Geometry::Line(_line) => todo!(),
                geo_types::Geometry::LineString(line_string) => {
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
                geo_types::Geometry::Polygon(_polygon) => todo!(),
                geo_types::Geometry::MultiPoint(multi_point) => {
                    for point in multi_point {
                        painter.circle_filled(
                            transformed_pos2(point.x(), point.y()),
                            3.0,
                            Color32::from_rgb(200, 200, 0),
                        );
                    }
                }
                geo_types::Geometry::MultiLineString(multi_line_string) => {
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
                geo_types::Geometry::MultiPolygon(multi_polygon) => {
                    for polygon in multi_polygon {
                        let points = polygon
                            .exterior()
                            .0
                            .iter()
                            .map(|p| transformed_pos2(p.x, p.y))
                            .collect::<Vec<_>>();
                        let stroke = egui::Stroke::new(2.0, Color32::GREEN.gamma_multiply(0.4));
                        painter.add(Shape::closed_line(points, stroke));
                    }
                }
                geo_types::Geometry::GeometryCollection(_geometry_collection) => todo!(),
                geo_types::Geometry::Rect(_rect) => todo!(),
                geo_types::Geometry::Triangle(_triangle) => todo!(),
            }
        }
    }
    Ok(())
}
