use ecolor::Color32;
use emath::pos2;
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
    render::{
        Coord, Error, Geometry,
        drawable::{Drawable, Mesh, Vertex},
    },
    style::Paint,
};

pub(crate) fn render(
    geometry: &Geometry<f32>,
    context: &Context,
    paint: &Paint,
    drawables: &mut Vec<Drawable>,
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

#[expect(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::geometry_type_to_str;
    use std::collections::HashMap;
    fn filled(geometry: Geometry<f32>) -> Vec<Drawable> {
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
        render(&geometry, &context, &paint, &mut drawables).unwrap();
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

        assert_eq!(filled(Geometry::Polygon(polygon.clone())).len(), 1);
        assert_eq!(
            filled(Geometry::MultiPolygon(geo_types::MultiPolygon::new(vec![
                polygon.clone(),
                polygon
            ])))
            .len(),
            2
        );
    }
}
