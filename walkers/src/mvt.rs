//! Renderer for Mapbox Vector Tiles.

use egui::{pos2, Color32, ColorImage, Context, Shape};
use tiny_skia::{Color, FillRule, Paint, Shader, Stroke, Transform};

use crate::Texture;

pub fn render2(tile: &mvt_reader::Reader, painter: &egui::Painter, rect: egui::Rect) {
    // Tile coords are from 0 to 4096, but we have a rect to fill.
    let transformed_pos2 = |x: f32, y: f32| {
        pos2(
            rect.left() + (x / 4096.0) * rect.width(),
            rect.top() + (y / 4096.0) * rect.height(),
        )
    };

    // That is just dumb, but mvt-reader API sucks.
    for (i, metadata) in tile.get_layer_metadata().unwrap().iter().enumerate() {
        assert_eq!(metadata.extent, 4096);

        for layer in tile.get_features(i) {
            for feature in layer {
                match feature.geometry {
                    geo_types::Geometry::Point(point) => todo!(),
                    geo_types::Geometry::Line(line) => todo!(),
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
                    geo_types::Geometry::Polygon(polygon) => todo!(),
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
                    geo_types::Geometry::GeometryCollection(geometry_collection) => todo!(),
                    geo_types::Geometry::Rect(rect) => todo!(),
                    geo_types::Geometry::Triangle(triangle) => todo!(),
                }
            }
        }
    }
}
