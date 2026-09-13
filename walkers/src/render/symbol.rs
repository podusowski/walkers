use ecolor::Color32;
use emath::pos2;
use log::warn;

use crate::{
    expression::Context,
    render::{Error, Geometry, Line},
    style::{Layout, Paint},
    text::{Placement, Text},
};

pub fn render(
    geometry: &Geometry<f32>,
    context: &Context,
    texts: &mut Vec<Text>,
    layout: &Layout,
    paint: &Option<Paint>,
) -> Result<(), Error> {
    match geometry {
        Geometry::Point(point) => {
            label_points(std::slice::from_ref(point), context, layout, paint, texts)
        }
        Geometry::MultiPoint(multi_point) => {
            label_points(&multi_point.0, context, layout, paint, texts)
        }
        Geometry::LineString(line_string) => label_line_strings(
            std::slice::from_ref(line_string),
            context,
            layout,
            paint,
            texts,
        ),
        Geometry::MultiLineString(multi_line_string) => {
            label_line_strings(&multi_line_string.0, context, layout, paint, texts)
        }
        _ => (),
    }
    Ok(())
}

/// Put labels on top of points.
fn label_points(
    points: &[geo_types::Point<f32>],
    context: &Context,
    layout: &Layout,
    paint: &Option<Paint>,
    texts: &mut Vec<Text>,
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

/// Put labels on top of lines.
fn label_line_strings(
    line_strings: &[geo_types::LineString<f32>],
    context: &Context,
    layout: &Layout,
    paint: &Option<Paint>,
    texts: &mut Vec<Text>,
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

#[expect(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::geometry_type_to_str;
    use std::collections::HashMap;
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
        render(&geometry, &context, &mut texts, &layout, &None).unwrap();
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
}
