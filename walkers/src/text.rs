use egui::{Color32, Pos2, Vec2, vec2};

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
    pub corners: [Pos2; 4],
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

        Self {
            corners: [p0, p1, p2, p3],
        }
    }

    pub fn top_left(&self) -> Pos2 {
        self.corners[0]
    }

    pub fn intersects(&self, other: &OrientedRect) -> bool {
        // Separating Axis Theorem on the 4 candidate axes (2 from self, 2 from other)
        for axis in self.edges().into_iter().chain(other.edges()) {
            if axis.length_sq() == 0.0 {
                continue; // degenerate, skip
            }
            let (a_min, a_max) = OrientedRect::project_onto_axis(&self.corners, axis);
            let (b_min, b_max) = OrientedRect::project_onto_axis(&other.corners, axis);
            // If intervals don't overlap -> separating axis exists
            if a_max < b_min || b_max < a_min {
                return false;
            }
        }
        true
    }

    fn edges(&self) -> [egui::Vec2; 2] {
        // Two unique edge directions are enough for SAT for rectangles.
        [
            self.corners[1] - self.corners[0],
            self.corners[3] - self.corners[0],
        ]
    }

    fn project_onto_axis(points: &[egui::Pos2; 4], axis: egui::Vec2) -> (f32, f32) {
        // No need to normalize axis for interval overlap test
        let dot = |p: egui::Pos2| -> f32 { p.x * axis.x + p.y * axis.y };
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for &p in points {
            let d = dot(p);
            if d < min {
                min = d;
            }
            if d > max {
                max = d;
            }
        }
        (min, max)
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
