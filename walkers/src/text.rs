use egui::{Color32, Pos2, Vec2, vec2};
use geo::{BoundingRect, Intersects, bounding_rect};

#[derive(Debug, Clone)]
pub struct Text {
    pub text: String,
    pub position: Pos2,
    pub font_size: f32,
    pub text_color: Color32,
    pub background_color: Color32,
    pub angle: f32,
}

impl Text {
    pub fn new(
        position: Pos2,
        text: String,
        font_size: f32,
        text_color: Color32,
        background_color: Color32,
        angle: f32,
    ) -> Self {
        Self {
            position,
            text,
            font_size,
            text_color,
            background_color,
            angle,
        }
    }
}

pub struct OrientedRect {
    pub geometry: geo::Polygon<f32>,
    bbox: geo::Rect<f32>,
}

impl OrientedRect {
    pub fn new(center: Pos2, angle: f32, size: Vec2) -> Self {
        let (s, c) = angle.sin_cos();
        let half = size * 0.5;

        let ux = vec2(half.x * c, half.x * s);
        let uy = vec2(-half.y * s, half.y * c);

        let p0 = center - ux - uy; // top-left
        let p1 = center + ux - uy; // top-right
        let p2 = center + ux + uy; // bottom-right
        let p3 = center - ux + uy; // bottom-left

        let polygon = geo::Polygon::new(
            geo::LineString::from(vec![
                geo::Coordinate { x: p0.x, y: p0.y },
                geo::Coordinate { x: p1.x, y: p1.y },
                geo::Coordinate { x: p2.x, y: p2.y },
                geo::Coordinate { x: p3.x, y: p3.y },
                geo::Coordinate { x: p0.x, y: p0.y }, // close the loop
            ]),
            vec![],
        );

        let bounding_rect = polygon.bounding_rect().unwrap();

        Self {
            geometry: polygon,
            bbox: bounding_rect,
        }
    }

    pub fn top_left(&self) -> Pos2 {
        self.geometry
            .exterior()
            .points()
            .nth(0)
            .map(|p| Pos2 {
                x: p.x() as f32,
                y: p.y() as f32,
            })
            .unwrap()
    }

    pub fn intersects(&self, other: &OrientedRect) -> bool {
        self.bbox.intersects(&other.bbox) && self.geometry.intersects(&other.geometry)
    }
}

// Tracks areas occupied by texts to avoid overlapping them.
pub struct OccupiedAreas {
    areas: Vec<OrientedRect>,
}

impl OccupiedAreas {
    pub fn new() -> Self {
        Self { areas: Vec::new() }
    }

    pub fn try_occupy(&mut self, rect: OrientedRect) -> bool {
        if !self.areas.iter().any(|existing| existing.intersects(&rect)) {
            self.areas.push(rect);
            true
        } else {
            false
        }
    }
}
