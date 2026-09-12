use ecolor::Color32;
use emath::{Pos2, Rect, pos2};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub position: Pos2,
    pub color: Color32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// A rectangle of one color.
    pub fn rect(rect: Rect, color: Color32) -> Self {
        let corners = [
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
        ];

        Self {
            vertices: corners
                .into_iter()
                .map(|position| Vertex { position, color })
                .collect(),
            indices: vec![0, 1, 2, 0, 2, 3],
        }
    }

    /// Take another mesh into this one, moving its indices along to match.
    pub fn append(&mut self, other: &Mesh) {
        let offset = self.vertices.len() as u32;

        self.vertices.extend_from_slice(&other.vertices);
        self.indices
            .extend(other.indices.iter().map(|index| index + offset));
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub points: Vec<Pos2>,

    /// In screen pixels, so it does not follow the map's scaling.
    pub width: f32,

    pub color: Color32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Drawable {
    Fill(Mesh),
    Lines(Vec<Line>),
}

impl Drawable {
    pub fn background(size: f32, color: Color32) -> Self {
        Self::Fill(Mesh::rect(
            Rect::from_min_max(pos2(0., 0.), pos2(size, size)),
            color,
        ))
    }
}
