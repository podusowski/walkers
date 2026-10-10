use ecolor::Color32;
use emath::pos2;

use geo_types::LineString;

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

    render_inner(geometry, width, color, dasharray.as_deref(), drawables);

    Ok(())
}

fn render_inner(
    geometry: &Geometry<f32>,
    width: f32,
    color: Color32,
    dasharray: Option<&[f32]>,
    drawables: &mut Vec<Drawable>,
) {
    match geometry {
        Geometry::LineString(line_string) => {
            push_line(line_string, width, color, dasharray, drawables);
        }
        Geometry::MultiLineString(multi_line_string) => {
            for line_string in multi_line_string {
                push_line(line_string, width, color, dasharray, drawables);
            }
        }
        Geometry::GeometryCollection(collection) => {
            for geometry in collection {
                render_inner(geometry, width, color, dasharray, drawables);
            }
        }
        _ => (),
    }
}

fn push_line(
    line_string: &LineString<f32>,
    width: f32,
    color: Color32,
    dasharray: Option<&[f32]>,
    drawables: &mut Vec<Drawable>,
) {
    drawables.push(Drawable::Lines(vec![Line {
        points: line_string.0.iter().map(|p| pos2(p.x, p.y)).collect(),
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
