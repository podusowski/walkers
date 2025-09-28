//! Renderer for Mapbox Vector Tiles.

use egui::{
    emath::TSTransform,
    epaint::{CircleShape, PathShape, PathStroke},
    pos2, Color32, Pos2, Shape, Stroke,
};
use geo_types::Geometry;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Mvt(#[from] mvt_reader::error::ParserError),
}

/// Currently this is the only supported extent.
const ONLY_SUPPORTED_EXTENT: u32 = 4096;

/// Render MVT data into a list of [`epaint::Shape`]s.
pub fn render(data: &mvt_reader::Reader) -> Result<Vec<Shape>, Error> {
    let line_stroke = Stroke::new(3.0, Color32::WHITE);
    let mut shapes = Vec::new();

    for index in supported_layers(data) {
        for feature in data.get_features(index)? {
            match feature.geometry {
                Geometry::Point(_point) => todo!(),
                Geometry::Line(_line) => todo!(),
                Geometry::LineString(line_string) => {
                    for segment in line_string.0.windows(2) {
                        shapes.push(Shape::line_segment(
                            [
                                pos2(segment[0].x, segment[0].y),
                                pos2(segment[1].x, segment[1].y),
                            ],
                            line_stroke,
                        ));
                    }
                }
                Geometry::Polygon(_polygon) => todo!(),
                Geometry::MultiPoint(multi_point) => {
                    for point in multi_point {
                        shapes.push(
                            CircleShape {
                                center: pos2(point.x(), point.y()),
                                radius: 3.0,
                                fill: Color32::from_rgb(200, 200, 0),
                                stroke: Stroke::NONE,
                            }
                            .into(),
                        );
                    }
                }
                Geometry::MultiLineString(multi_line_string) => {
                    for line_string in multi_line_string {
                        let points = line_string
                            .0
                            .iter()
                            .map(|p| pos2(p.x, p.y))
                            .collect::<Vec<_>>();
                        shapes.push(Shape::line(points, line_stroke));
                    }
                }
                Geometry::MultiPolygon(multi_polygon) => {
                    for polygon in multi_polygon.iter() {
                        let points = polygon
                            .exterior()
                            .0
                            .iter()
                            .map(|p| pos2(p.x, p.y))
                            .collect::<Vec<_>>();
                        shapes.extend(arbitrary_polygon(&points));
                    }
                }
                Geometry::GeometryCollection(_geometry_collection) => todo!(),
                Geometry::Rect(_rect) => todo!(),
                Geometry::Triangle(_triangle) => todo!(),
            }
        }
    }

    Ok(shapes)
}

/// Transform shapes from MVT space to screen space.
pub fn transformed(shapes: &[Shape], rect: egui::Rect) -> Vec<Shape> {
    shapes
        .iter()
        .map(|shape| {
            let mut shape = shape.clone();
            shape.transform(TSTransform {
                scaling: rect.width() / ONLY_SUPPORTED_EXTENT as f32,
                translation: rect.min.to_vec2(),
            });
            shape
        })
        .collect()
}

fn supported_layers(data: &mvt_reader::Reader) -> impl Iterator<Item = usize> + '_ {
    data.get_layer_metadata()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|layer| {
            if layer.extent == ONLY_SUPPORTED_EXTENT {
                Some(layer.layer_index)
            } else {
                log::warn!(
                    "Skipping layer '{}' with unsupported extent {}.",
                    layer.name,
                    layer.extent
                );
                None
            }
        })
}

/// Egui can only draw convex polygons, so we need to triangulate arbitrary ones.
fn arbitrary_polygon(points: &[Pos2]) -> Vec<Shape> {
    let mut shapes = Vec::new();
    let mut triangles = Vec::<usize>::new();
    let mut earcut = earcut::Earcut::new();
    earcut.earcut(points.iter().map(|p| [p.x, p.y]), &[], &mut triangles);

    for triangle_indices in triangles.chunks(3) {
        let triangle = [
            points[triangle_indices[0]],
            points[triangle_indices[1]],
            points[triangle_indices[2]],
        ];

        if triangle_area(triangle[0], triangle[1], triangle[2]) < 100.0 {
            // Too small to render without artifacts.
            continue;
        }

        shapes.push(
            PathShape::convex_polygon(
                triangle.to_vec(),
                Color32::WHITE.gamma_multiply(0.2),
                PathStroke::NONE,
            )
            .into(),
        );
    }
    shapes
}

fn triangle_area(a: Pos2, b: Pos2, c: Pos2) -> f32 {
    ((a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y)) / 2.0).abs()
}
