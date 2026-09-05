//! Turning geometries into something drawable, the way a [`crate::Style`] says to, and then
//! drawing it.
//!
//! Where the geometries came from - vector tiles, GeoJSON, KML - is not this module's concern.

/// What there is to draw, in a form no particular renderer is baked into.
pub mod drawable;

/// Drawing it.
#[cfg(feature = "mvt")]
pub mod gpu;

use ecolor::Color32;
use emath::{TSTransform, pos2};
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
    render::drawable::{Drawable, Line as DrawableLine, Mesh, Vertex},
    style::{Layout, Paint},
    text::{Placement, Text},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Tessellation(#[from] TessellationError),
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

/// Things of the same kind standing next to each other can be drawn as one. Consecutive is as
/// far as this can go: a line drawn between two fills has to stay between them, so a run ends
/// wherever something of another kind does.
pub(crate) fn merge_runs(drawables: Vec<Drawable>) -> Vec<Drawable> {
    let mut merged: Vec<Drawable> = Vec::with_capacity(drawables.len());

    for drawable in drawables {
        match (merged.last_mut(), drawable) {
            (Some(Drawable::Fill(run)), Drawable::Fill(mesh)) => run.append(&mesh),
            (Some(Drawable::Lines(run)), Drawable::Lines(lines)) => run.extend(lines),
            (_, drawable) => merged.push(drawable),
        }
    }

    merged
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
    drawables: &mut Vec<Drawable>,
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

    match geometry {
        Geometry::LineString(line_string) => {
            let points = line_string
                .0
                .iter()
                .map(|p| pos2(p.x, p.y))
                .collect::<Vec<_>>();
            push_line(drawables, points, width, color, dasharray.as_deref());
        }
        Geometry::MultiLineString(multi_line_string) => {
            for line_string in multi_line_string {
                let points = line_string
                    .0
                    .iter()
                    .map(|p| pos2(p.x, p.y))
                    .collect::<Vec<_>>();
                push_line(drawables, points, width, color, dasharray.as_deref());
            }
        }
        _ => (),
    }

    Ok(())
}

/// Push a polyline as one or more lines, splitting it into dashes if `dasharray` is given.
fn push_line(
    drawables: &mut Vec<Drawable>,
    points: Vec<emath::Pos2>,
    width: f32,
    color: Color32,
    dasharray: Option<&[f32]>,
) {
    let mut push = |points: Vec<emath::Pos2>| {
        drawables.push(Drawable::Lines(vec![DrawableLine {
            points,
            width,
            color,
        }]))
    };

    match dasharray {
        Some(pattern) if !pattern.is_empty() => {
            for segment in dash_polyline(&points, pattern, width) {
                if segment.len() >= 2 {
                    push(segment);
                }
            }
        }
        _ => push(points),
    }
}

/// Split a polyline into the "on" (dash) runs of a dash/gap `pattern`, whose values are
/// in units of `width` per the MapLibre `line-dasharray` spec. Each returned run is a
/// standalone polyline.
fn dash_polyline(points: &[emath::Pos2], pattern: &[f32], width: f32) -> Vec<Vec<emath::Pos2>> {
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
    drawables: &mut Vec<Drawable>,
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
        drawables.push(Drawable::Fill(tessellate_polygon(
            &exterior, &interiors, fill_color,
        )?));
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
    let (halo_color, halo_width) = evaluate_halo(paint, context);

    texts.extend(points.iter().map(|p| {
        Text::new(pos2(p.x(), p.y()), text.clone(), text_size, text_color, 0.0)
            .with_halo(halo_color, halo_width)
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
    let (halo_color, halo_width) = evaluate_halo(paint, context);

    for line_string in line_strings {
        let lines: Vec<_> = line_string.lines().collect();

        // Use the longest line to fit the label.
        if let Some(line) = lines.into_iter().max_by_key(|line| length(line) as u32) {
            let mid_point = midpoint(&line.start_point(), &line.end_point());
            let angle = line.slope().atan();

            texts.push(
                Text::new(
                    pos2(mid_point.x(), mid_point.y()),
                    text.clone(),
                    text_size,
                    text_color,
                    angle,
                )
                .with_halo(halo_color, halo_width)
                .with_placement(Placement::Line),
            );
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

/// A halo is only drawn where the style asks for one; the spec has no width by default.
fn evaluate_halo(paint: &Option<Paint>, context: &Context) -> (Color32, f32) {
    let Some(paint) = paint else {
        return (Color32::TRANSPARENT, 0.0);
    };

    let color = match &paint.text_halo_color {
        Some(color) => color.evaluate(context),
        None => return (Color32::TRANSPARENT, 0.0),
    };

    let width = match &paint.text_halo_width {
        Some(width) => width.evaluate(context),
        None => 0.0,
    };

    (color, width)
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
            let position = vertex.position();
            Vertex {
                position: pos2(position.x, position.y),
                color: fill_color,
            }
        }),
    )?;

    Ok(Mesh {
        indices: buffers.indices,
        vertices: buffers.vertices,
    })
}

/// Convert list of `geo_types::Coord` to Lyon's `Point`s.
fn lyon_points(points: &[Coord<f32>]) -> Vec<Point<f32>> {
    points.iter().map(|p| point(p.x, p.y)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled(vertices: usize) -> Drawable {
        Drawable::Fill(Mesh {
            vertices: vec![
                Vertex {
                    position: pos2(0., 0.),
                    color: Color32::WHITE,
                };
                vertices
            ],
            indices: (0..vertices as u32).collect(),
        })
    }

    fn vertices_of(drawable: &Drawable) -> usize {
        match drawable {
            Drawable::Fill(mesh) => mesh.vertices.len(),
            Drawable::Lines(_) => 0,
        }
    }

    fn line() -> Drawable {
        Drawable::Lines(vec![DrawableLine {
            points: vec![pos2(0., 0.), pos2(1., 1.)],
            width: 1.,
            color: Color32::WHITE,
        }])
    }

    #[test]
    fn fills_standing_next_to_each_other_become_one() {
        let merged = merge_runs(vec![filled(3), filled(4), filled(5)]);

        assert_eq!(merged.len(), 1);
        assert_eq!(vertices_of(&merged[0]), 12);
    }

    /// Merging across a line would move the fills on top of it.
    #[test]
    fn a_line_between_two_fills_keeps_them_apart() {
        let merged = merge_runs(vec![filled(3), line(), filled(4)]);

        assert_eq!(merged.len(), 3);
        assert_eq!(vertices_of(&merged[0]), 3);
        assert!(matches!(merged[1], Drawable::Lines(_)));
        assert_eq!(vertices_of(&merged[2]), 4);
    }

    /// Thousands of lines in a tile, and a renderer wants them as few pieces of work.
    #[test]
    fn lines_standing_next_to_each_other_become_one_run() {
        let merged = merge_runs(vec![line(), line(), line()]);

        assert_eq!(merged.len(), 1);
        match &merged[0] {
            Drawable::Lines(run) => assert_eq!(run.len(), 3),
            other => panic!("expected a run of lines, got {other:?}"),
        }
    }
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

    fn fill(geometry: Geometry<f32>) -> Vec<Drawable> {
        let context = Context::new(
            geometry_type_to_str(&geometry).to_string(),
            HashMap::new(),
            12,
        );

        let paint = Paint {
            fill_color: Some(crate::style::Color(crate::style::json!("#ff0000"))),
            ..Default::default()
        };

        let mut drawables = Vec::new();
        render_polygon(&geometry, &context, &mut drawables, &paint).unwrap();
        drawables
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
