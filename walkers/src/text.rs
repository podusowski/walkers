use egui::{Color32, Pos2, Shape, Vec2, vec2};
use geo::{BoundingRect, Coord, Intersects, LineString, Polygon};

#[derive(Debug, Clone)]
pub struct Text {
    pub text: String,
    pub position: Pos2,
    pub font_size: f32,
    pub text_color: Color32,
    pub halo_color: Color32,
    pub halo_width: f32,
    pub angle: f32,
}

impl Text {
    pub fn new(
        position: Pos2,
        text: String,
        font_size: f32,
        text_color: Color32,
        angle: f32,
    ) -> Self {
        Self {
            position,
            text,
            font_size,
            text_color,
            halo_color: Color32::TRANSPARENT,
            halo_width: 0.0,
            angle,
        }
    }

    pub fn with_halo(mut self, color: Color32, width: f32) -> Self {
        self.halo_color = color;
        self.halo_width = width;
        self
    }
}

/// Where to stamp a copy of the text to fake an outline around it.
const HALO_RING: [Vec2; 8] = [
    Vec2 { x: 1.0, y: 0.0 },
    Vec2 { x: 0.7, y: 0.7 },
    Vec2 { x: 0.0, y: 1.0 },
    Vec2 { x: -0.7, y: 0.7 },
    Vec2 { x: -1.0, y: 0.0 },
    Vec2 { x: -0.7, y: -0.7 },
    Vec2 { x: 0.0, y: -1.0 },
    Vec2 { x: 0.7, y: -0.7 },
];

pub struct OrientedRect {
    polygon: Polygon<f32>,
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

        let polygon = Polygon::new(
            LineString::from(vec![
                Coord { x: p0.x, y: p0.y },
                Coord { x: p1.x, y: p1.y },
                Coord { x: p2.x, y: p2.y },
                Coord { x: p3.x, y: p3.y },
                Coord { x: p0.x, y: p0.y }, // Close the polygon
            ]),
            vec![],
        );

        let bounding_rect = polygon
            .bounding_rect()
            .expect("can not happen because polygon always has some points");

        Self {
            polygon,
            bbox: bounding_rect,
        }
    }

    pub fn top_left(&self) -> Pos2 {
        self.polygon
            .exterior()
            .points()
            .nth(0)
            .map(|p| Pos2 { x: p.x(), y: p.y() })
            .expect("can not happen because polygon always has some points")
    }

    pub fn intersects(&self, other: &OrientedRect) -> bool {
        // Checking bbox first gives huge performance boost.
        self.bbox.intersects(&other.bbox) && self.polygon.intersects(&other.polygon)
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

/// Lay the text out, unless it would land on an area which is already taken.
fn place_text(text: Text, ctx: &egui::Context, occupied_text_areas: &mut OccupiedAreas) -> Shape {
    use egui::epaint::TextShape;

    let mut layout_job = egui::text::LayoutJob::default();

    layout_job.append(
        &text.text,
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::proportional(text.font_size),
            color: text.text_color,
            ..Default::default()
        },
    );

    let galley = ctx.fonts_mut(|fonts| fonts.layout_job(layout_job));

    let area = OrientedRect::new(text.position, text.angle, galley.size());
    let top_left = area.top_left();

    if !occupied_text_areas.try_occupy(area) {
        return Shape::Noop;
    }

    let on_top =
        TextShape::new(top_left, galley.to_owned(), text.text_color).with_angle(text.angle);

    if text.halo_width <= 0.0 || text.halo_color.a() == 0 {
        return on_top.into();
    }

    // Text cannot be stroked, so the halo is the same text stamped around it.
    let mut shapes: Vec<Shape> = HALO_RING
        .iter()
        .map(|offset| {
            TextShape::new(
                top_left + *offset * text.halo_width,
                galley.to_owned(),
                text.halo_color,
            )
            .with_angle(text.angle)
            .with_override_text_color(text.halo_color)
            .into()
        })
        .collect();

    shapes.push(on_top.into());
    Shape::Vec(shapes)
}

/// Lay out the labels, dropping the ones which would land on top of an already placed one.
pub fn place_texts(texts: Vec<Text>, ctx: &egui::Context) -> Vec<Shape> {
    let mut occupied_text_areas = OccupiedAreas::new();

    // Need to collect it to avoid deadlock caused by `Painter::extend` and `fonts_mut`.
    texts
        .into_iter()
        .map(|text| place_text(text, ctx, &mut occupied_text_areas))
        .collect()
}
