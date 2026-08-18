use egui::{Color32, Pos2, Shape, Vec2, vec2};
use geo::{BoundingRect, Coord, Intersects, LineString, Polygon};
use std::collections::HashMap;

/// What the label names, which decides whether it may be said again nearby. Mirrors MapLibre's
/// `symbol-placement`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Placement {
    #[default]
    Point,
    Line,
}

#[derive(Debug, Clone)]
pub struct Text {
    pub text: String,
    pub position: Pos2,
    pub font_size: f32,
    pub text_color: Color32,
    pub halo_color: Color32,
    pub halo_width: f32,
    pub angle: f32,
    pub placement: Placement,
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
            placement: Placement::Point,
        }
    }

    pub fn with_halo(mut self, color: Color32, width: f32) -> Self {
        self.halo_color = color;
        self.halo_width = width;
        self
    }

    pub fn with_placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
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

/// How far apart two labels saying the same thing have to be.
const MIN_REPEAT_DISTANCE: f32 = 400.0;

/// What has been placed so far - where it sits, and what it says.
///
/// Only line labels are remembered by name. OSM splits a way at every intersection, so one
/// street really is many features saying the same thing, while two points sharing a name are two
/// different things - house numbers being the obvious case.
#[derive(Default)]
struct PlacedTexts {
    occupied_areas: Vec<OrientedRect>,
    text_positions: HashMap<String, Vec<Pos2>>,
}

impl PlacedTexts {
    fn already_placed_nearby(&self, text: &Text) -> bool {
        text.placement == Placement::Line
            && self
                .text_positions
                .get(&text.text)
                .is_some_and(|positions| {
                    positions
                        .iter()
                        .any(|placed| placed.distance(text.position) < MIN_REPEAT_DISTANCE)
                })
    }

    /// Take the area, unless something is already there. The text is remembered only once it
    /// turned out to be free.
    fn try_place(&mut self, text: &Text, area: OrientedRect) -> bool {
        if self
            .occupied_areas
            .iter()
            .any(|occupied| occupied.intersects(&area))
        {
            return false;
        }

        self.occupied_areas.push(area);

        if text.placement == Placement::Line {
            self.text_positions
                .entry(text.text.to_owned())
                .or_default()
                .push(text.position);
        }

        true
    }
}

/// Lay the text out, unless it repeats a nearby one or would land on an area which is already
/// taken.
fn place_text(text: Text, ctx: &egui::Context, placed_texts: &mut PlacedTexts) -> Shape {
    use egui::epaint::TextShape;

    // Before laying it out, which is the expensive part.
    if placed_texts.already_placed_nearby(&text) {
        return Shape::Noop;
    }

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

    if !placed_texts.try_place(&text, area) {
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

/// Lay out the labels, dropping the ones which repeat a nearby label or would land on top of an
/// already placed one.
pub fn place_texts(texts: Vec<Text>, ctx: &egui::Context) -> Vec<Shape> {
    let mut placed_texts = PlacedTexts::default();

    // Need to collect it to avoid deadlock caused by `Painter::extend` and `fonts_mut`.
    texts
        .into_iter()
        .map(|text| place_text(text, ctx, &mut placed_texts))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    fn context() -> egui::Context {
        let ctx = egui::Context::default();
        // Fonts only exist once a pass has been run.
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();
        ctx
    }

    /// Stacked vertically, so that whatever rejects them is the repeat rule rather than the
    /// collision one.
    fn column(name: &str, spacing: f32, placement: Placement) -> Vec<Text> {
        (0..2)
            .map(|n| {
                Text::new(
                    pos2(0., n as f32 * spacing),
                    name.to_string(),
                    12.,
                    Color32::BLACK,
                    0.,
                )
                .with_placement(placement)
            })
            .collect()
    }

    fn count_placed(texts: Vec<Text>, ctx: &egui::Context) -> usize {
        place_texts(texts, ctx)
            .into_iter()
            .filter(|shape| !matches!(shape, Shape::Noop))
            .count()
    }

    #[test]
    fn a_street_name_is_not_repeated_within_the_repeat_distance() {
        let ctx = context();
        let texts = column("Szczepankowice", MIN_REPEAT_DISTANCE / 2., Placement::Line);

        assert_eq!(count_placed(texts, &ctx), 1);
    }

    #[test]
    fn a_street_name_is_said_again_once_far_enough_away() {
        let ctx = context();
        let texts = column("Szczepankowice", MIN_REPEAT_DISTANCE * 2., Placement::Line);

        assert_eq!(count_placed(texts, &ctx), 2);
    }

    #[test]
    fn different_street_names_may_sit_close_together() {
        let ctx = context();
        let mut texts = column("Szczepankowice", MIN_REPEAT_DISTANCE / 2., Placement::Line);
        texts[1].text = "Wrocław".to_string();

        assert_eq!(count_placed(texts, &ctx), 2);
    }

    /// Two houses may well share a number, so unlike a street split into many features, they
    /// are not the same thing said twice.
    #[test]
    fn neighbouring_houses_keep_the_same_number() {
        let ctx = context();
        let texts = column("5", MIN_REPEAT_DISTANCE / 2., Placement::Point);

        assert_eq!(count_placed(texts, &ctx), 2);
    }
}
