//! Renderer for Mapbox Vector Tiles.

use egui::{
    epaint::{PathShape, PathStroke},
    pos2, Color32, Pos2, Shape,
};

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
            let Some(properties) = feature.properties else {
                continue;
            };

            let Some(kind) = properties.get("kind") else {
                continue;
            };

            //match kind {
            //    mvt_reader::feature::Value::String(s) if s == "building" => {}
            //    _ => {
            //        //    println!("Unknown kind: {:?}", kind);
            //        continue;
            //    }
            //}

            //if feature.id != Some(35184472817089)
            //{
            //    continue;
            //}

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
                    for (i, polygon) in multi_polygon.iter().enumerate() {
                       // if i != 172 {
                       //     continue;
                       // }
                        let points = polygon
                            .exterior()
                            .0
                            .iter()
                            .map(|p| transformed_pos2(p.x, p.y))
                            .collect::<Vec<_>>();
                        //let stroke = egui::Stroke::new(2.0, Color32::GREEN.gamma_multiply(0.4));
                        //painter.add(Shape::closed_line(points, stroke));
                        arbitrary_polygon(&points, painter);
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

/// Egui can only draw convex polygons, so we need to triangulate arbitrary ones.
fn arbitrary_polygon(points: &[Pos2], painter: &egui::Painter) {
    let flat_points = points.iter().flat_map(|p| [p.x, p.y]).collect::<Vec<_>>();
    //let triangles = earcutr::earcut(&flat_points, &[], 2).unwrap();

    let mut triangles = Vec::<usize>::new();
    let mut earcut = earcut::Earcut::new();
    earcut.earcut(points.iter().map(|p| [p.x, p.y]), &[], &mut triangles);

    for (i, triangle_indices) in triangles.chunks(3).enumerate() {
        //if i != 1 {
        //    continue;
        //}

        let triangle = [
            points[triangle_indices[0]],
            points[triangle_indices[1]],
            points[triangle_indices[2]],
        ];

        if triangle_area(triangle[0], triangle[1], triangle[2]) < 1.0 {
            // too small
            continue;
        }

        //println!("Triangle {i}: {:?}", triangle);
        painter.add(PathShape::convex_polygon(
            triangle.to_vec(),
            Color32::from_rgb(100, 150, 200).gamma_multiply(0.5),
            PathStroke::new(1.0, Color32::RED),
        ));

        for point in triangle {
            painter.circle_filled((point).into(), 3.0, Color32::BLUE);
        }
    }
}

fn triangle_area(a: Pos2, b: Pos2, c: Pos2) -> f32 {
    ((a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y)) / 2.0).abs()
}
