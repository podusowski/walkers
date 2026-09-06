use ecolor::Color32;
use emath::pos2;

use crate::{
    expression::Context,
    render::{
        Error, Geometry,
        drawable::{Drawable, Line},
    },
    style::Paint,
};

pub fn render(
    geometry: &Geometry<f32>,
    context: &Context,
    paint: &Paint,
    drawables: &mut Vec<Drawable>,
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
            push_line(points, width, color, dasharray.as_deref(), drawables);
        }
        Geometry::MultiLineString(multi_line_string) => {
            for line_string in multi_line_string {
                let points = line_string
                    .0
                    .iter()
                    .map(|p| pos2(p.x, p.y))
                    .collect::<Vec<_>>();
                push_line(points, width, color, dasharray.as_deref(), drawables);
            }
        }
        _ => (),
    }

    Ok(())
}

/// Push a polyline as one or more lines, splitting it into dashes if `dasharray` is given.
fn push_line(
    points: Vec<emath::Pos2>,
    width: f32,
    color: Color32,
    dasharray: Option<&[f32]>,
    drawables: &mut Vec<Drawable>,
) {
    let mut push = |points: Vec<emath::Pos2>| {
        drawables.push(Drawable::Lines(vec![Line {
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

#[cfg(test)]
mod tests {
    use super::*;
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
}
