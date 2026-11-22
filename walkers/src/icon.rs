//! Symbols drawn as shapes rather than as glyphs, so that they need no font. Whether an emoji is
//! drawn depends on the fonts an app gives egui: it may come in colour, ignoring the style's, or
//! not at all, as ⛰ was in egui's own fonts.
//!
//! The shapes are the SVG files in `icons/`, which `build.rs` turns into their outlines and
//! triangles, for `lookup_icon_outline`.

use egui::{Color32, Pos2, Shape, Stroke, epaint, pos2};

/// `lookup_icon_outline`, from what `build.rs` read out of `icons/`.
macro_rules! icons {
    ($(
        $name:literal => {
            rings: $rings:expr,
            vertices: $vertices:expr,
            indices: $indices:expr $(,)?
        }
    )*) => {
        #[doc = concat!(
            "The outline of a Walkers' own icon, by the name a style's `icon-image` gives it, or ",
            "`None` for one Walkers does not have. These are the SVG files in `icons/` of ",
            "Walkers' sources:\n\n",
            $("* `", $name, "`\n",)*
        )]
        pub fn lookup_icon_outline(name: &str) -> Option<&'static IconOutline> {
            match name {
                $($name => Some(const {
                    &IconOutline {
                        name: $name,
                        rings: $rings,
                        vertices: $vertices,
                        indices: $indices,
                    }
                }),)*
                _ => None,
            }
        }
    };
}

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

/// An image built into Walkers, centred on the origin and one across. Each is of one colour, as a
/// MapLibre SDF icon is, so `icon-color` and `icon-halo-*` apply to all of them.
pub struct IconOutline {
    name: &'static str,
    /// For its edges.
    rings: &'static [&'static [Pos2]],
    /// For its inside.
    vertices: &'static [Pos2],
    indices: &'static [u32],
}

impl IconOutline {
    /// What a style's `icon-image` calls it.
    pub fn name(&self) -> &'static str {
        self.name
    }
}

/// By name, rather than as the hundreds of points it is.
impl std::fmt::Debug for IconOutline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("IconOutline").field(&self.name).finish()
    }
}

impl PartialEq for IconOutline {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

/// An image as a label draws it, on the label's position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Icon {
    pub outline: &'static IconOutline,
    /// Width and height, in points.
    pub size: f32,
    pub color: Color32,
    pub halo_color: Color32,
    pub halo_width: f32,
}

impl Icon {
    /// Points across an icon of `icon-size` 1.
    pub const SIZE: f32 = 14.0;

    pub fn new(outline: &'static IconOutline, size: f32, color: Color32) -> Self {
        Self {
            outline,
            size,
            color,
            halo_color: Color32::TRANSPARENT,
            halo_width: 0.0,
        }
    }

    pub fn with_halo(mut self, color: Color32, width: f32) -> Self {
        self.halo_color = color;
        self.halo_width = width;
        self
    }

    /// The icon, centred on `center`.
    pub(crate) fn shapes(&self, center: Pos2, pixels_per_point: f32) -> Vec<Shape> {
        let outline = self.outline;
        let on_screen = |p: Pos2| center + p.to_vec2() * self.size;
        let rings: Vec<Vec<Pos2>> = outline
            .rings
            .iter()
            .map(|ring| ring.iter().copied().map(on_screen).collect())
            .collect();

        let mut shapes = Vec::new();

        // Each edge stroked twice as wide as the halo, half of it then hidden under the inside.
        if self.halo_width > 0.0 && self.halo_color.a() > 0 {
            shapes.extend(rings.iter().map(|ring| {
                Shape::closed_line(
                    ring.clone(),
                    Stroke::new(2.0 * self.halo_width, self.halo_color),
                )
            }));
        }

        shapes.push(Shape::mesh(epaint::Mesh {
            indices: outline.indices.to_vec(),
            vertices: outline
                .vertices
                .iter()
                .map(|&vertex| epaint::Vertex {
                    pos: on_screen(vertex),
                    uv: epaint::WHITE_UV,
                    color: self.color,
                })
                .collect(),
            ..Default::default()
        }));

        // egui smooths the edges of lines, not of meshes. A line of a pixel round the edges gives
        // the icon the soft edge a glyph has.
        shapes.extend(
            rings.into_iter().map(|ring| {
                Shape::closed_line(ring, Stroke::new(1.0 / pixels_per_point, self.color))
            }),
        );

        shapes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every outline, found by the name of its file.
    fn all() -> Vec<&'static IconOutline> {
        std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/icons"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "svg"))
            .map(|path| {
                let name = path.file_stem().unwrap().to_str().unwrap();
                lookup_icon_outline(name).unwrap_or_else(|| panic!("no outline {name}"))
            })
            .collect()
    }

    #[test]
    fn every_file_of_icons_is_an_outline_by_its_name() {
        for outline in all() {
            assert_eq!(lookup_icon_outline(outline.name()), Some(outline));
        }
        assert_eq!(lookup_icon_outline("mountain_11"), None);
    }

    #[test]
    fn every_outline_has_an_inside_within_its_size() {
        for outline in all() {
            assert!(!outline.indices.is_empty(), "{outline:?}");
            for vertex in outline.vertices {
                assert!(
                    vertex.x.abs() <= 0.5 && vertex.y.abs() <= 0.5,
                    "{outline:?} reaches {vertex:?}"
                );
            }
        }
    }

    /// The windows of a train are not painted over.
    #[test]
    fn a_ring_inside_another_is_a_hole() {
        let outline = lookup_icon_outline("train").unwrap();
        let window_middle = pos2(0.0, (5.75 - 8.0) / 16.0);
        let covered = outline.indices.chunks(3).any(|triangle| {
            let [a, b, c] = [0, 1, 2].map(|k| outline.vertices[triangle[k] as usize]);
            contains([a, b, c], window_middle)
        });
        assert!(!covered);
    }

    fn contains([a, b, c]: [Pos2; 3], p: Pos2) -> bool {
        let side = |u: Pos2, v: Pos2| (v - u).x * (p - u).y - (v - u).y * (p - u).x;
        let (d1, d2, d3) = (side(a, b), side(b, c), side(c, a));
        let negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
        let positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
        !(negative && positive)
    }

    #[test]
    fn an_icon_is_drawn_in_its_colour_round_its_center() {
        let icon = Icon::new(lookup_icon_outline("peak").unwrap(), 14.0, Color32::RED);
        let shapes = icon.shapes(pos2(100.0, 50.0), 1.0);

        let meshes: Vec<_> = shapes
            .iter()
            .filter_map(|shape| match shape {
                Shape::Mesh(mesh) => Some(mesh),
                _ => None,
            })
            .collect();
        assert_eq!(meshes.len(), 1);
        for vertex in &meshes[0].vertices {
            assert_eq!(vertex.color, Color32::RED);
            assert!((vertex.pos.x - 100.0).abs() <= 7.0 && (vertex.pos.y - 50.0).abs() <= 7.0);
        }
    }

    #[test]
    fn a_halo_is_drawn_under_the_icon() {
        let icon = Icon::new(lookup_icon_outline("peak").unwrap(), 14.0, Color32::RED)
            .with_halo(Color32::WHITE, 1.0);
        let shapes = icon.shapes(pos2(0.0, 0.0), 1.0);

        let Some(Shape::Path(halo)) = shapes.first() else {
            panic!("expected the halo first, got {shapes:?}");
        };
        assert_eq!(halo.stroke.width, 2.0);
        assert!(shapes.iter().position(|s| matches!(s, Shape::Mesh(_))) > Some(0));
    }
}
