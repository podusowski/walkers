//! Turning what a tile decoded into ([`crate::drawable`]) into something egui can paint.
//!
//! This is the one place which knows both, and the first thing another renderer would need
//! its own version of.

use egui::{
    Shape, Stroke,
    emath::TSTransform,
    epaint::{Vertex, WHITE_UV},
};

use crate::drawable::Drawable;

/// A tile's drawables, in the form egui wants them. Done once, when the tile is decoded,
/// rather than on every frame it is visible on.
pub fn to_shapes(drawables: &[Drawable]) -> Vec<Shape> {
    drawables
        .iter()
        .map(|drawable| match drawable {
            Drawable::Fill(mesh) => Shape::Mesh(
                egui::Mesh {
                    vertices: mesh
                        .vertices
                        .iter()
                        .map(|vertex| Vertex {
                            pos: vertex.position,
                            uv: WHITE_UV,
                            color: vertex.color,
                        })
                        .collect(),
                    indices: mesh.indices.to_owned(),
                    ..Default::default()
                }
                .into(),
            ),
            Drawable::Line(line) => {
                Shape::line(line.points.to_owned(), Stroke::new(line.width, line.color))
            }
        })
        .collect()
}

pub(crate) fn transformed_shape(shape: &Shape, transform: TSTransform) -> Shape {
    let mut shape = shape.to_owned();
    shape.transform(transform);

    // `line-width` is in screen pixels, so a stroke must not follow the scaling.
    keep_stroke_width(&mut shape, transform.scaling);

    shape
}

pub(crate) fn transformed_shapes(shapes: &[Shape], transform: TSTransform) -> Vec<Shape> {
    shapes
        .iter()
        .map(|shape| transformed_shape(shape, transform))
        .collect()
}

/// Undo what [`Shape::transform`] did to the stroke widths, which it scales along with
/// everything else.
fn keep_stroke_width(shape: &mut Shape, scaling: f32) {
    if scaling == 0.0 {
        return;
    }

    match shape {
        Shape::Vec(shapes) => {
            for shape in shapes {
                keep_stroke_width(shape, scaling);
            }
        }
        Shape::Path(path) => path.stroke.width /= scaling,
        Shape::LineSegment { stroke, .. } => stroke.width /= scaling,
        Shape::Circle(circle) => circle.stroke.width /= scaling,
        Shape::Ellipse(ellipse) => ellipse.stroke.width /= scaling,
        Shape::Rect(rect) => rect.stroke.width /= scaling,
        Shape::QuadraticBezier(curve) => curve.stroke.width /= scaling,
        Shape::CubicBezier(curve) => curve.stroke.width /= scaling,
        Shape::Noop | Shape::Text(_) | Shape::Mesh(_) | Shape::Callback(_) => {}
    }
}

#[cfg(test)]
mod width_tests {
    use super::*;
    use crate::expression::Context;
    use crate::render::{Geometry, render_line};
    use crate::style::{Color, Float, Paint, json};

    fn line_width_after(scaling: f32, asked: f32) -> f32 {
        let context = Context::new("LineString".to_string(), Default::default(), 10);
        let paint = Paint {
            line_color: Some(Color(json!("#000000"))),
            line_width: Some(Float(json!(asked))),
            ..Default::default()
        };
        let geometry = Geometry::LineString(geo_types::LineString::from(vec![
            (0.0f32, 0.0f32),
            (100.0, 100.0),
        ]));

        let mut drawables = Vec::new();
        render_line(&geometry, &context, &mut drawables, &paint).unwrap();

        let shapes = transformed_shapes(
            &to_shapes(&drawables),
            TSTransform {
                scaling,
                translation: Default::default(),
            },
        );

        shapes
            .iter()
            .find_map(|shape| match shape {
                Shape::LineSegment { stroke, .. } => Some(stroke.width),
                Shape::Path(path) => Some(path.stroke.width),
                _ => None,
            })
            .expect("no line")
    }

    /// A tile is drawn at whatever size the current zoom calls for, but `line-width` is in
    /// screen pixels and must not follow it.
    #[test]
    fn line_width_survives_the_transform() {
        for scaling in [256.0 / 4096.0, 362.0 / 4096.0, 511.0 / 4096.0, 1.0] {
            let width = line_width_after(scaling, 4.0);
            assert!(
                (width - 4.0).abs() < 0.001,
                "asked for 4.0, got {width} at scaling {scaling}"
            );
        }
    }

    #[test]
    fn geometry_still_scales() {
        let context = Context::new("LineString".to_string(), Default::default(), 10);
        let geometry = Geometry::LineString(geo_types::LineString::from(vec![
            (0.0f32, 0.0f32),
            (4096.0, 0.0),
        ]));

        let mut drawables = Vec::new();
        render_line(&geometry, &context, &mut drawables, &Paint::default()).unwrap();
        let shapes = transformed_shapes(
            &to_shapes(&drawables),
            TSTransform {
                scaling: 256.0 / 4096.0,
                translation: Default::default(),
            },
        );

        let points = match &shapes[0] {
            Shape::LineSegment { points, .. } => points.to_vec(),
            Shape::Path(path) => path.points.to_vec(),
            other => panic!("expected a line, got {other:?}"),
        };

        // The full extent of the tile lands on the full width it is drawn at.
        assert_eq!(points[0].x, 0.0);
        assert_eq!(points[points.len() - 1].x, 256.0);
    }
}
