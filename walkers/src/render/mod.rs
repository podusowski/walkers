//! Working out what a [`crate::Style`] says to draw, and drawing it.

pub mod drawable;
pub mod fill;
pub mod line;
pub mod symbol;

#[cfg(feature = "mvt")]
pub mod gpu;

use emath::TSTransform;
pub use geo_types::{Coord, Geometry, Line};
use lyon_tessellation::TessellationError;

use crate::{render::drawable::Drawable, text::Text};

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

/// Things of the same kind standing next to each other can be drawn as one.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::drawable::{Line as DrawableLine, Mesh, Vertex};
    use ecolor::Color32;
    use emath::pos2;

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
}
