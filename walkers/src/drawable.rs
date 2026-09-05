//! What a tile turns into, before any particular renderer gets hold of it.
//!
//! Nothing in here knows how it will be drawn. A renderer takes these and does whatever it
//! does - hands them to egui as shapes, uploads them to the GPU, writes them out as an image.
//! Positions are in the pixels of a tile, so a renderer still has to place the tile itself.

use ecolor::Color32;
use emath::{Pos2, Rect, pos2};

/// One corner of a triangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub position: Pos2,
    pub color: Color32,
}

/// An area, already cut into triangles, because working out how to cut it is the same job
/// whoever is drawing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// A rectangle of one colour, which is how a background layer arrives.
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

/// A line, left as points rather than as a stroked outline, so that whoever draws it can do so
/// the way it does lines - on the CPU, or by extruding it in a shader.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub points: Vec<Pos2>,

    /// In screen pixels, so it does not follow the map's scaling.
    pub width: f32,

    pub color: Color32,
}

/// One thing to draw, in the order it should be drawn.
#[derive(Clone, Debug, PartialEq)]
pub enum Drawable {
    Fill(Mesh),
    Line(Line),
}

impl Drawable {
    /// A one-colour rectangle covering the whole tile.
    pub fn background(size: f32, color: Color32) -> Self {
        Self::Fill(Mesh::rect(
            Rect::from_min_max(pos2(0., 0.), pos2(size, size)),
            color,
        ))
    }
}
