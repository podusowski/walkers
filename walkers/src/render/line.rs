use ecolor::Color32;
use emath::pos2;

use geo_types::{LineString, Polygon};

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
        Geometry::Polygon(polygon) => {
            push_rings(polygon, width, color, dasharray, drawables);
        }
        Geometry::MultiPolygon(multi_polygon) => {
            for polygon in multi_polygon {
                push_rings(polygon, width, color, dasharray, drawables);
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

/// The outline of a polygon: its exterior, and the edges of its holes.
fn push_rings(
    polygon: &Polygon<f32>,
    width: f32,
    color: Color32,
    dasharray: Option<&[f32]>,
    drawables: &mut Vec<Drawable>,
) {
    for ring in std::iter::once(polygon.exterior()).chain(polygon.interiors()) {
        push_line(ring, width, color, dasharray, drawables);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::geometry_type_to_str;
    use geo_types::MultiPolygon;
    use std::collections::HashMap;

    fn lines(geometry: Geometry<f32>) -> Vec<Line> {
        let context = Context::new(
            geometry_type_to_str(&geometry).to_string(),
            HashMap::new(),
            12,
        );

        let mut drawables = Vec::new();
        render(&geometry, &context, &Paint::default(), &mut drawables).unwrap();
        drawables
            .into_iter()
            .flat_map(|drawable| match drawable {
                Drawable::Lines(lines) => lines,
                Drawable::Fill(_) => panic!("a line layer filled something"),
            })
            .collect()
    }

    fn square(from: f32, to: f32) -> LineString<f32> {
        LineString::from(vec![
            (from, from),
            (to, from),
            (to, to),
            (from, to),
            (from, from),
        ])
    }

    #[test]
    fn polygons_are_outlined_along_every_ring() {
        let polygon = Polygon::new(square(0., 10.), vec![square(2., 4.), square(6., 8.)]);

        let outlined = lines(Geometry::Polygon(polygon.clone()));
        assert_eq!(outlined.len(), 3);
        assert_eq!(outlined[0].points.len(), 5);

        assert_eq!(
            lines(Geometry::MultiPolygon(MultiPolygon::new(vec![
                polygon.clone(),
                polygon
            ])))
            .len(),
            6
        );
    }
}
