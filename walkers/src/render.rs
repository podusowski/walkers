//! Draw geometries the way a [`crate::Style`] says to.
//!
//! Where the geometries came from - vector tiles, GeoJSON, KML - is not this module's concern.

use egui::{
    Color32, Mesh, Shape, Stroke,
    emath::TSTransform,
    epaint::{Vertex, WHITE_UV},
    pos2,
};
pub use geo_types::{Coord, Geometry, Line};
use log::warn;
use lyon_path::{
    Path, Polygon,
    geom::{Point, point},
};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, TessellationError, VertexBuffers,
};

use crate::{
    expression::Context,
    style::{Layout, Paint},
    text::Text,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Tessellation(#[from] TessellationError),
}

pub(crate) fn transformed_shapes(shapes: &[Shape], transform: TSTransform) -> Vec<Shape> {
    let mut shapes = shapes.to_vec();

    for shape in shapes.iter_mut() {
        shape.transform(transform);

        // `line-width` is in screen pixels, so a stroke must not follow the scaling.
        keep_stroke_width(shape, transform.scaling);
    }

    shapes
}

pub(crate) fn transformed_texts(texts: &[Text], transform: TSTransform) -> Vec<Text> {
    texts
        .iter()
        .map(|text| Text {
            position: text.position * transform.scaling + transform.translation,
            ..text.to_owned()
        })
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

pub(crate) fn geometry_type_to_str(geometry: &Geometry<f32>) -> &'static str {
    match geometry {
        Geometry::Point(_) | Geometry::MultiPoint(_) => "Point",
        Geometry::Line(_) => "Line",
        Geometry::LineString(_) | Geometry::MultiLineString(_) => "LineString",
        Geometry::Polygon(_) | Geometry::MultiPolygon(_) => "Polygon",
        Geometry::GeometryCollection(_) => "GeometryCollection",
        Geometry::Rect(_) => "Rect",
        Geometry::Triangle(_) => "Triangle",
    }
}

pub fn render_line(
    geometry: &Geometry<f32>,
    context: &Context,
    shapes: &mut Vec<Shape>,
    paint: &Paint,
) -> Result<(), Error> {
    let width = if let Some(width) = &paint.line_width {
        width.evaluate(context)
    } else {
        1.0
    };

    let opacity = if let Some(opacity) = &paint.line_opacity {
        opacity.evaluate(context)
    } else {
        1.0
    };

    let color = if let Some(color) = &paint.line_color {
        color.evaluate(context).gamma_multiply(opacity)
    } else {
        Color32::WHITE
    };

    let dasharray = paint
        .line_dasharray
        .as_ref()
        .and_then(|dasharray| dasharray.evaluate(context));

    let stroke = Stroke::new(width, color);

    match geometry {
        Geometry::LineString(line_string) => {
            let points = line_string
                .0
                .iter()
                .map(|p| pos2(p.x, p.y))
                .collect::<Vec<_>>();
            push_line(shapes, points, stroke, dasharray.as_deref());
        }
        Geometry::MultiLineString(multi_line_string) => {
            for line_string in multi_line_string {
                let points = line_string
                    .0
                    .iter()
                    .map(|p| pos2(p.x, p.y))
                    .collect::<Vec<_>>();
                push_line(shapes, points, stroke, dasharray.as_deref());
            }
        }
        _ => (),
    }

    Ok(())
}

/// Push a polyline as one or more shapes, splitting it into dashes if `dasharray` is given.
fn push_line(
    shapes: &mut Vec<Shape>,
    points: Vec<egui::Pos2>,
    stroke: Stroke,
    dasharray: Option<&[f32]>,
) {
    match dasharray {
        Some(pattern) if !pattern.is_empty() => {
            for segment in dash_polyline(&points, pattern, stroke.width) {
                if segment.len() >= 2 {
                    shapes.push(Shape::line(segment, stroke));
                }
            }
        }
        _ => shapes.push(Shape::line(points, stroke)),
    }
}

/// Split a polyline into the "on" (dash) runs of a dash/gap `pattern`, whose values are
/// in units of `width` per the MapLibre `line-dasharray` spec. Each returned run is a
/// standalone polyline meant to be drawn as its own [`Shape::line`].
fn dash_polyline(points: &[egui::Pos2], pattern: &[f32], width: f32) -> Vec<Vec<egui::Pos2>> {
    let pattern = pattern
        .iter()
        .map(|value| value * width)
        .collect::<Vec<_>>();

    if points.len() < 2 || pattern.iter().sum::<f32>() <= 0.0 {
        return vec![points.to_vec()];
    }

    let mut segments = Vec::new();
    let mut current = vec![points[0]];
    let mut pattern_index = 0;
    let mut remaining = pattern[0];
    let mut drawing = true;

    for window in points.windows(2) {
        let (mut start, end) = (window[0], window[1]);
        let mut length = start.distance(end);

        while length > 0.0 {
            if remaining >= length {
                remaining -= length;
                if drawing {
                    current.push(end);
                }
                length = 0.0;
            } else {
                let point = start + (end - start) * (remaining / length);

                if drawing {
                    current.push(point);
                    segments.push(std::mem::take(&mut current));
                } else {
                    current = vec![point];
                }

                length -= remaining;
                start = point;
                drawing = !drawing;
                pattern_index = (pattern_index + 1) % pattern.len();
                remaining = pattern[pattern_index];
            }
        }
    }

    if drawing && current.len() > 1 {
        segments.push(current);
    }

    segments
}

pub(crate) fn render_polygon(
    geometry: &Geometry<f32>,
    context: &Context,
    shapes: &mut Vec<Shape>,
    paint: &Paint,
) -> Result<(), Error> {
    let polygons: &[geo_types::Polygon<f32>] = match geometry {
        Geometry::Polygon(polygon) => std::slice::from_ref(polygon),
        Geometry::MultiPolygon(multi_polygon) => &multi_polygon.0,
        _ => return Ok(()),
    };

    let Some(fill_color) = &paint.fill_color else {
        warn!("Fill layer without fill color. Skipping.");
        return Ok(());
    };

    let fill_color = fill_color.evaluate(context);

    let fill_color = if let Some(fill_opacity) = &paint.fill_opacity {
        let fill_opacity = fill_opacity.evaluate(context);
        fill_color.gamma_multiply(fill_opacity)
    } else {
        fill_color
    };

    for polygon in polygons {
        let exterior = lyon_points(&polygon.exterior().0);
        let interiors = polygon
            .interiors()
            .iter()
            .map(|hole| lyon_points(&hole.0))
            .collect::<Vec<_>>();
        shapes.push(Shape::Mesh(
            tessellate_polygon(&exterior, &interiors, fill_color)?.into(),
        ));
    }

    Ok(())
}

pub fn render_symbol(
    geometry: &Geometry<f32>,
    context: &Context,
    texts: &mut Vec<Text>,
    layout: &Layout,
    paint: &Option<Paint>,
) -> Result<(), Error> {
    match geometry {
        Geometry::Point(point) => {
            label_points(std::slice::from_ref(point), context, texts, layout, paint)
        }
        Geometry::MultiPoint(multi_point) => {
            label_points(&multi_point.0, context, texts, layout, paint)
        }
        Geometry::LineString(line_string) => label_line_strings(
            std::slice::from_ref(line_string),
            context,
            texts,
            layout,
            paint,
        ),
        Geometry::MultiLineString(multi_line_string) => {
            label_line_strings(&multi_line_string.0, context, texts, layout, paint)
        }
        _ => (),
    }
    Ok(())
}

fn label_points(
    points: &[geo_types::Point<f32>],
    context: &Context,
    texts: &mut Vec<Text>,
    layout: &Layout,
    paint: &Option<Paint>,
) {
    let Some(text) = layout.text(context) else {
        return;
    };

    let text_size = evaluate_text_size(layout, context);
    let text_color = evaluate_text_color(paint, context);

    texts.extend(points.iter().map(|p| {
        Text::new(
            pos2(p.x(), p.y()),
            text.clone(),
            text_size,
            text_color,
            Color32::TRANSPARENT,
            0.0,
        )
    }))
}

fn label_line_strings(
    line_strings: &[geo_types::LineString<f32>],
    context: &Context,
    texts: &mut Vec<Text>,
    layout: &Layout,
    paint: &Option<Paint>,
) {
    let Some(text) = layout.text(context) else {
        return;
    };

    let text_size = evaluate_text_size(layout, context);
    let text_color = evaluate_text_color(paint, context);

    let text_halo_color = if let Some(paint) = paint
        && let Some(color) = &paint.text_halo_color
    {
        color.evaluate(context)
    } else {
        Color32::TRANSPARENT
    };

    for line_string in line_strings {
        let lines: Vec<_> = line_string.lines().collect();

        // Use the longest line to fit the label.
        if let Some(line) = lines.into_iter().max_by_key(|line| length(line) as u32) {
            let mid_point = midpoint(&line.start_point(), &line.end_point());
            let angle = line.slope().atan();

            texts.push(Text::new(
                pos2(mid_point.x(), mid_point.y()),
                text.clone(),
                text_size,
                text_color,
                // TODO: Implement real halo rendering.
                text_halo_color.gamma_multiply(0.5),
                angle,
            ));
        }
    }
}

fn evaluate_text_size(layout: &Layout, context: &Context) -> f32 {
    layout
        .text_size
        .as_ref()
        .and_then(|text_size| {
            let size = text_size.evaluate(context);

            if size > 3.0 {
                Some(size)
            } else {
                warn!(
                    "{} evaluated into {size}, which is too small for text size.",
                    text_size.0
                );
                None
            }
        })
        // Default from MapLibre spec.
        .unwrap_or(12.0)
}

fn evaluate_text_color(paint: &Option<Paint>, context: &Context) -> Color32 {
    if let Some(paint) = paint
        && let Some(color) = &paint.text_color
    {
        color.evaluate(context)
    } else {
        // Default from MapLibre spec.
        Color32::BLACK
    }
}

fn length(line: &Line<f32>) -> f32 {
    (line.dx() * line.dx() + line.dy() * line.dy()).sqrt()
}

fn midpoint(p1: &geo_types::Point<f32>, p2: &geo_types::Point<f32>) -> geo_types::Point<f32> {
    geo_types::Point::new((p1.x() + p2.x()) / 2.0, (p1.y() + p2.y()) / 2.0)
}

/// Egui cannot tessellate complex polygons, so we use lyon for that.
pub fn tessellate_polygon(
    exterior: &[Point<f32>],
    interiors: &[Vec<Point<f32>>],
    fill_color: Color32,
) -> Result<Mesh, TessellationError> {
    let mut builder = Path::builder();

    builder.add_polygon(Polygon {
        points: exterior,
        closed: true,
    });

    for interior in interiors {
        builder.add_polygon(Polygon {
            points: interior,
            closed: true,
        });
    }

    let mut buffers: VertexBuffers<Vertex, u32> = VertexBuffers::new();

    FillTessellator::new().tessellate_path(
        &builder.build(),
        &FillOptions::default(),
        &mut BuffersBuilder::new(&mut buffers, |vertex: FillVertex| {
            let pos = vertex.position();
            Vertex {
                pos: pos2(pos.x, pos.y),
                uv: WHITE_UV,
                color: fill_color,
            }
        }),
    )?;

    Ok(Mesh {
        indices: buffers.indices,
        vertices: buffers.vertices,
        ..Default::default()
    })
}

/// Convert list of `geo_types::Coord` to Lyon's `Point`s.
fn lyon_points(points: &[Coord<f32>]) -> Vec<Point<f32>> {
    points.iter().map(|p| point(p.x, p.y)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn dash_polyline_without_pattern_keeps_a_single_segment() {
        let points = vec![pos2(0.0, 0.0), pos2(10.0, 0.0)];
        let segments = dash_polyline(&points, &[], 1.0);
        assert_eq!(segments, vec![points]);
    }

    #[test]
    fn dash_polyline_splits_dashes_and_gaps() {
        // Pattern [2, 1] at width 1.0 means a 2-unit dash followed by a 1-unit gap,
        // repeating. Over a 10-unit straight line that's dash/gap/dash/gap/dash/gap...
        let points = vec![pos2(0.0, 0.0), pos2(9.0, 0.0)];
        let segments = dash_polyline(&points, &[2.0, 1.0], 1.0);

        assert_eq!(
            segments,
            vec![
                vec![pos2(0.0, 0.0), pos2(2.0, 0.0)],
                vec![pos2(3.0, 0.0), pos2(5.0, 0.0)],
                vec![pos2(6.0, 0.0), pos2(8.0, 0.0)],
                vec![pos2(9.0, 0.0)],
            ]
            .into_iter()
            .filter(|segment: &Vec<egui::Pos2>| segment.len() >= 2)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn dash_polyline_pattern_scales_with_line_width() {
        let points = vec![pos2(0.0, 0.0), pos2(4.0, 0.0)];
        // width 2.0 turns the [2, 1] pattern into 4-unit dash, 2-unit gap.
        let segments = dash_polyline(&points, &[2.0, 1.0], 2.0);
        assert_eq!(segments, vec![vec![pos2(0.0, 0.0), pos2(4.0, 0.0)]]);
    }

    fn label(geometry: Geometry<f32>) -> Vec<Text> {
        let context = Context::new(
            geometry_type_to_str(&geometry).to_string(),
            HashMap::from([("name".to_string(), serde_json::Value::from("Śnieżka"))]),
            12,
        );

        let layout = Layout {
            text_field: Some(crate::style::json!(["get", "name"])),
            text_size: None,
        };

        let mut texts = Vec::new();
        render_symbol(&geometry, &context, &mut texts, &layout, &None).unwrap();
        texts
    }

    fn texts(texts: &[Text]) -> Vec<&str> {
        texts.iter().map(|text| text.text.as_str()).collect()
    }

    /// MVT hands over `MultiPoint`, GeoJSON a plain `Point`, and both need labelling.
    #[test]
    fn points_are_labelled_whether_they_come_singly_or_not() {
        let point = geo_types::Point::new(1.0, 2.0);

        assert_eq!(texts(&label(Geometry::Point(point))), ["Śnieżka"]);
        assert_eq!(
            texts(&label(Geometry::MultiPoint(
                vec![point, geo_types::Point::new(3.0, 4.0)].into()
            ))),
            ["Śnieżka", "Śnieżka"]
        );
    }

    #[test]
    fn line_strings_are_labelled_whether_they_come_singly_or_not() {
        let line_string =
            geo_types::LineString::from(vec![(0.0f32, 0.0f32), (10.0, 0.0), (10.0, 10.0)]);

        assert_eq!(
            texts(&label(Geometry::LineString(line_string.clone()))),
            ["Śnieżka"]
        );
        assert_eq!(
            texts(&label(Geometry::MultiLineString(
                geo_types::MultiLineString::new(vec![line_string.clone(), line_string])
            ))),
            ["Śnieżka", "Śnieżka"]
        );
    }

    /// Labels are placed on the geometry, not at the origin.
    #[test]
    fn a_point_label_lands_on_the_point() {
        let texts = label(Geometry::Point(geo_types::Point::new(1.0, 2.0)));

        match texts.as_slice() {
            [text] => assert_eq!(text.position, pos2(1.0, 2.0)),
            other => panic!("expected a single label, got {other:?}"),
        }
    }

    fn fill(geometry: Geometry<f32>) -> Vec<Shape> {
        let context = Context::new(
            geometry_type_to_str(&geometry).to_string(),
            HashMap::new(),
            12,
        );

        let paint = Paint {
            fill_color: Some(crate::style::Color(crate::style::json!("#ff0000"))),
            ..Default::default()
        };

        let mut shapes = Vec::new();
        render_polygon(&geometry, &context, &mut shapes, &paint).unwrap();
        shapes
    }

    /// A tile with one ring per feature gives a plain `Polygon`, not a `MultiPolygon`.
    #[test]
    fn polygons_are_filled_whether_they_come_singly_or_not() {
        let polygon = geo_types::Polygon::new(
            geo_types::LineString::from(vec![
                (0.0f32, 0.0f32),
                (10.0, 0.0),
                (10.0, 10.0),
                (0.0, 0.0),
            ]),
            vec![],
        );

        assert_eq!(fill(Geometry::Polygon(polygon.clone())).len(), 1);
        assert_eq!(
            fill(Geometry::MultiPolygon(geo_types::MultiPolygon::new(vec![
                polygon.clone(),
                polygon
            ])))
            .len(),
            2
        );
    }
}

#[cfg(test)]
mod width_tests {
    use super::*;
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

        let mut shapes = Vec::new();
        render_line(&geometry, &context, &mut shapes, &paint).unwrap();

        let shapes = transformed_shapes(
            &shapes,
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

        let mut shapes = Vec::new();
        render_line(&geometry, &context, &mut shapes, &Paint::default()).unwrap();
        let shapes = transformed_shapes(
            &shapes,
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
