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

fn push_line(
    points: Vec<emath::Pos2>,
    width: f32,
    color: Color32,
    dasharray: Option<&[f32]>,
    drawables: &mut Vec<Drawable>,
) {
    drawables.push(Drawable::Lines(vec![Line {
        points,
        width,
        color,
        // A style gives dashes in line widths.
        dasharray: dasharray
            .unwrap_or_default()
            .iter()
            .map(|value| value * width)
            .collect(),
    }]))
}
