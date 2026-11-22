use egui::{Color32, Pos2, Rect, Shape, Vec2, vec2};
use std::collections::HashMap;
use std::sync::Arc;

use crate::icon::Icon;

/// What the label names, which decides whether it may be said again nearby. Mirrors MapLibre's
/// `symbol-placement`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Placement {
    #[default]
    Point,
    Line,
}

/// Which part of a label sits on its position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Anchor {
    #[default]
    Center,
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Anchor {
    /// The anchor a style names, as `text-anchor` does.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "center" => Self::Center,
            "left" => Self::Left,
            "right" => Self::Right,
            "top" => Self::Top,
            "bottom" => Self::Bottom,
            "top-left" => Self::TopLeft,
            "top-right" => Self::TopRight,
            "bottom-left" => Self::BottomLeft,
            "bottom-right" => Self::BottomRight,
            _ => return None,
        })
    }

    /// Where the middle of a label of `size` is, from its anchor, in the label's own axes.
    fn to_center(self, size: Vec2) -> Vec2 {
        let half = size / 2.0;
        let x = match self {
            Self::Left | Self::TopLeft | Self::BottomLeft => half.x,
            Self::Right | Self::TopRight | Self::BottomRight => -half.x,
            _ => 0.0,
        };
        let y = match self {
            Self::Top | Self::TopLeft | Self::TopRight => half.y,
            Self::Bottom | Self::BottomLeft | Self::BottomRight => -half.y,
            _ => 0.0,
        };
        vec2(x, y)
    }
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
    pub min_zoom: Option<f32>,
    pub layer: usize,
    pub padding: f32,
    pub max_width: Option<f32>,
    pub anchor: Anchor,
    pub offset: Vec2,
    /// Drawn centred on `position`, with the text placed from there as without it. The two are
    /// placed together, or neither is. With an icon, an empty text is no text at all.
    pub icon: Option<Icon>,
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
            min_zoom: None,
            layer: 0,
            padding: 0.0,
            max_width: None,
            anchor: Anchor::Center,
            offset: Vec2::ZERO,
            icon: None,
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

    pub fn with_min_zoom(mut self, min_zoom: Option<f32>) -> Self {
        self.min_zoom = min_zoom;
        self
    }

    pub fn with_layer(mut self, layer: usize) -> Self {
        self.layer = layer;
        self
    }

    pub fn with_padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    pub fn with_max_width(mut self, max_width: Option<f32>) -> Self {
        self.max_width = max_width;
        self
    }

    pub fn with_anchor(mut self, anchor: Anchor, offset: Vec2) -> Self {
        self.anchor = anchor;
        self.offset = offset;
        self
    }

    pub fn with_icon(mut self, icon: Option<Icon>) -> Self {
        self.icon = icon;
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

/// The area a label takes on the screen: a rectangle, turned with the label.
#[derive(Debug, Clone, Copy)]
pub struct OrientedRect {
    /// Top-left, top-right, bottom-right, bottom-left, before turning.
    corners: [Pos2; 4],
    bbox: Rect,
}

impl OrientedRect {
    pub fn new(center: Pos2, angle: f32, size: Vec2) -> Self {
        let (s, c) = angle.sin_cos();
        let half = size * 0.5;

        let ux = vec2(half.x * c, half.x * s);
        let uy = vec2(-half.y * s, half.y * c);

        let corners = [
            center - ux - uy,
            center + ux - uy,
            center + ux + uy,
            center - ux + uy,
        ];

        Self {
            corners,
            bbox: Rect::from_points(&corners),
        }
    }

    pub fn top_left(&self) -> Pos2 {
        self.corners[0]
    }

    /// Whether the two overlap, touching included.
    pub fn intersects(&self, other: &OrientedRect) -> bool {
        !self.separates(other) && !other.separates(self)
    }

    /// Whether one of this rectangle's edges is the direction of an axis on which the two do
    /// not overlap. Two convex polygons are apart exactly if some edge of either is such an
    /// axis (the separating axis theorem), and a rectangle has two directions of edges.
    fn separates(&self, other: &OrientedRect) -> bool {
        let [p0, p1, _, p3] = self.corners;
        [p1 - p0, p3 - p0].into_iter().any(|axis| {
            let (a_min, a_max) = self.extent_along(axis);
            let (b_min, b_max) = other.extent_along(axis);
            a_max < b_min || b_max < a_min
        })
    }

    /// The span of the corners projected on `axis`.
    fn extent_along(&self, axis: Vec2) -> (f32, f32) {
        self.corners
            .iter()
            .map(|p| p.to_vec2().dot(axis))
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), d| {
                (lo.min(d), hi.max(d))
            })
    }
}

impl rstar::RTreeObject for OrientedRect {
    type Envelope = rstar::AABB<[f32; 2]>;

    fn envelope(&self) -> Self::Envelope {
        rstar::AABB::from_corners(self.bbox.min.into(), self.bbox.max.into())
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
    /// In an R-tree, so that a new label is checked only against the ones near it.
    occupied_areas: rstar::RTree<OrientedRect>,
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
        // A label with no finite position or angle, such as an empty one on a line of no length,
        // whose angle is atan(0 / 0), could not be drawn anyway, and the tree cannot sort it.
        if !area.corners.iter().all(|corner| corner.is_finite()) {
            return false;
        }

        if self
            .occupied_areas
            .locate_in_envelope_intersecting(rstar::RTreeObject::envelope(&area))
            .any(|occupied| occupied.intersects(&area))
        {
            return false;
        }

        self.occupied_areas.insert(area);

        if text.placement == Placement::Line {
            self.text_positions
                .entry(text.text.to_owned())
                .or_default()
                .push(text.position);
        }

        true
    }
}

/// Lay the label out, unless it repeats a nearby one or would land on an area which is already
/// taken.
fn place_text(text: Text, ctx: &egui::Context, placed_texts: &mut PlacedTexts) -> Shape {
    // Before laying it out, which is the expensive part.
    if placed_texts.already_placed_nearby(&text) {
        return Shape::Noop;
    }

    // A label of no text still takes the height of a line, unless it is an icon's.
    let label = if text.icon.is_some() && text.text.is_empty() {
        None
    } else {
        match lay_out(&text, ctx) {
            Some(label) => Some(label),
            None => return Shape::Noop,
        }
    };

    if !placed_texts.try_place(&text, area(&text, label.as_ref())) {
        return Shape::Noop;
    }

    let Some(icon) = &text.icon else {
        return label.map_or(Shape::Noop, |label| label.shape(&text));
    };
    let mut shapes = icon.shapes(text.position, ctx.pixels_per_point());
    shapes.extend(label.map(|label| label.shape(&text)));
    Shape::Vec(shapes)
}

/// The text of a label, laid out and put where its anchor and offset say.
struct LaidOut {
    galley: Arc<egui::Galley>,
    top_left: Pos2,
    /// Of the area the text takes, padding included, turned with the label.
    center: Pos2,
    size: Vec2,
}

/// The one area a label takes: its text's, grown to hold its icon, turned with the text.
fn area(text: &Text, label: Option<&LaidOut>) -> OrientedRect {
    let Some(icon) = &text.icon else {
        return label.map_or(
            OrientedRect::new(text.position, text.angle, Vec2::ZERO),
            |l| OrientedRect::new(l.center, text.angle, l.size),
        );
    };

    // What the label draws, in its own axes. The icon is not turned with the text.
    let to_label = |p: Pos2| rotated(p.to_vec2(), -text.angle).to_pos2();
    let half_icon = Vec2::splat(icon.size / 2.0 + text.padding);
    let mut bounds = Rect::from_points(
        &[vec2(-1., -1.), vec2(1., -1.), vec2(1., 1.), vec2(-1., 1.)]
            .map(|corner| to_label(text.position + corner * half_icon)),
    );
    if let Some(label) = label {
        bounds = bounds.union(Rect::from_center_size(to_label(label.center), label.size));
    }

    OrientedRect::new(
        rotated(bounds.center().to_vec2(), text.angle).to_pos2(),
        text.angle,
        bounds.size(),
    )
}

/// `None` if the text is wider than it may be.
fn lay_out(text: &Text, ctx: &egui::Context) -> Option<LaidOut> {
    // Lines of a label are centred on each other.
    let mut layout_job = egui::text::LayoutJob {
        halign: egui::Align::Center,
        ..Default::default()
    };

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

    if text
        .max_width
        .is_some_and(|max_width| galley.size().x > max_width)
    {
        return None;
    }

    let center =
        text.position + text.offset + rotated(text.anchor.to_center(galley.size()), text.angle);
    // Centred lines put the galley's own origin in their middle, not at its top-left.
    let top_left = OrientedRect::new(center, text.angle, galley.size()).top_left()
        - rotated(galley.rect.min.to_vec2(), text.angle);
    let size = galley.size() + Vec2::splat(2.0 * text.padding);

    Some(LaidOut {
        galley,
        top_left,
        center,
        size,
    })
}

impl LaidOut {
    fn shape(&self, text: &Text) -> Shape {
        use egui::epaint::TextShape;

        let on_top = TextShape::new(self.top_left, self.galley.to_owned(), text.text_color)
            .with_angle(text.angle);

        if text.halo_width <= 0.0 || text.halo_color.a() == 0 {
            return on_top.into();
        }

        // Text cannot be stroked, so the halo is the same text stamped around it.
        let mut shapes: Vec<Shape> = HALO_RING
            .iter()
            .map(|offset| {
                TextShape::new(
                    self.top_left + *offset * text.halo_width,
                    self.galley.to_owned(),
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
}

/// `vector` turned by `angle`, in radians.
fn rotated(vector: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    vec2(
        vector.x * cos - vector.y * sin,
        vector.x * sin + vector.y * cos,
    )
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
    use rand::{RngExt as _, SeedableRng as _, rngs::StdRng};

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

    /// The text shape `text` is drawn as, with no halo to complicate it.
    fn placed(text: Text, ctx: &egui::Context) -> egui::epaint::TextShape {
        match place_texts(vec![text], ctx).as_slice() {
            [Shape::Text(shape)] => shape.clone(),
            other => panic!("expected a single text, got {other:?}"),
        }
    }

    /// Where the top-left corner of `text` lands once placed, and how big it is.
    fn placed_top_left(text: Text, ctx: &egui::Context) -> (Pos2, Vec2) {
        let shape = placed(text, ctx);
        (
            shape.pos + shape.galley.rect.min.to_vec2(),
            shape.galley.size(),
        )
    }

    /// Such as a peak's height under its name.
    #[test]
    fn lines_of_a_label_are_centred_on_each_other() {
        let ctx = context();
        let text = Text::new(
            pos2(100., 100.),
            "Ślęża\n718 m".into(),
            12.,
            Color32::BLACK,
            0.,
        );

        let shape = placed(text.clone(), &ctx);
        let [name, height] = &shape.galley.rows[..] else {
            panic!("expected two lines, got {:?}", shape.galley.rows);
        };
        assert!((name.rect().center().x - height.rect().center().x).abs() < 0.5);

        let (top_left, size) = placed_top_left(text, &ctx);
        assert_eq!(top_left, pos2(100. - size.x / 2., 100. - size.y / 2.));
    }

    /// Such as the name of a peak, set under a symbol which stands on the peak itself.
    #[test]
    fn a_label_anchored_at_its_top_hangs_below_its_point() {
        let ctx = context();
        let text = Text::new(pos2(100., 100.), "Ślęża".into(), 12., Color32::BLACK, 0.);

        let (centered, size) = placed_top_left(text.clone(), &ctx);
        assert_eq!(centered, pos2(100. - size.x / 2., 100. - size.y / 2.));

        let (hanging, _) = placed_top_left(text.with_anchor(Anchor::Top, Vec2::ZERO), &ctx);
        assert_eq!(hanging, pos2(100. - size.x / 2., 100.));
    }

    #[test]
    fn a_label_is_moved_by_its_offset() {
        let ctx = context();
        let text = Text::new(pos2(100., 100.), "Ślęża".into(), 12., Color32::BLACK, 0.);

        let (at_point, _) = placed_top_left(text.clone(), &ctx);
        let (moved, _) = placed_top_left(text.with_anchor(Anchor::Center, vec2(5., 10.)), &ctx);
        assert_eq!(moved, at_point + vec2(5., 10.));
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

    fn peak() -> Icon {
        Icon::new(
            crate::lookup_icon_outline("peak").unwrap(),
            14.,
            Color32::RED,
        )
    }

    /// Such as a peak with neither a name nor a height.
    #[test]
    fn an_icon_stands_alone_on_its_point() {
        let ctx = context();
        let text = Text::new(pos2(100., 100.), String::new(), 12., Color32::BLACK, 0.)
            .with_icon(Some(peak()));

        let shapes = place_texts(vec![text], &ctx);
        let [Shape::Vec(shapes)] = shapes.as_slice() else {
            panic!("expected the icon's shapes, got {shapes:?}");
        };
        assert!(shapes.iter().any(|s| matches!(s, Shape::Mesh(_))));
        assert!(!shapes.iter().any(|s| matches!(s, Shape::Text(_))));
    }

    #[test]
    fn an_icon_is_drawn_under_its_text() {
        let ctx = context();
        let text = Text::new(pos2(100., 100.), "Ślęża".into(), 12., Color32::BLACK, 0.)
            .with_anchor(Anchor::Top, vec2(0., 8.))
            .with_icon(Some(peak()));

        let shapes = place_texts(vec![text], &ctx);
        let [Shape::Vec(shapes)] = shapes.as_slice() else {
            panic!("expected the icon's and the text's shapes, got {shapes:?}");
        };
        let mesh = shapes.iter().position(|s| matches!(s, Shape::Mesh(_)));
        let text = shapes.iter().position(|s| matches!(s, Shape::Text(_)));
        assert!(
            mesh.is_some() && text.is_some() && mesh < text,
            "{shapes:?}"
        );
    }

    /// A name hanging under no peak, or a peak which nothing names, would both be wrong.
    #[test]
    fn an_icon_and_its_text_are_dropped_together() {
        let ctx = context();
        // Where the name hangs, under the peak, but clear of the peak itself.
        let in_the_way = Text::new(pos2(100., 115.), "Sobótka".into(), 12., Color32::BLACK, 0.);
        let named_peak = Text::new(pos2(100., 100.), "Ślęża".into(), 12., Color32::BLACK, 0.)
            .with_anchor(Anchor::Top, vec2(0., 8.))
            .with_icon(Some(peak()));

        assert_eq!(count_placed(vec![in_the_way.clone()], &ctx), 1);
        assert_eq!(count_placed(vec![named_peak.clone()], &ctx), 1);
        assert_eq!(count_placed(vec![in_the_way, named_peak], &ctx), 1);
    }

    /// Hung under its peak with a gap, a name and its peak still take one rectangle, holding
    /// both and the gap between them.
    #[test]
    fn an_icon_and_its_text_take_one_rectangle_round_both() {
        let ctx = context();
        let named_peak = Text::new(pos2(100., 100.), "Ślęża".into(), 12., Color32::BLACK, 0.)
            .with_anchor(Anchor::Top, vec2(0., 20.))
            .with_icon(Some(peak()));
        let text_alone = Text {
            icon: None,
            ..named_peak.clone()
        };
        let (text_top_left, text_size) = placed_top_left(text_alone, &ctx);

        let mut placed_texts = PlacedTexts::default();
        assert!(!matches!(
            place_text(named_peak, &ctx, &mut placed_texts),
            Shape::Noop
        ));

        let areas: Vec<_> = placed_texts.occupied_areas.iter().collect();
        let [area] = areas.as_slice() else {
            panic!("expected one area, got {}", areas.len());
        };
        assert_eq!(area.bbox.top(), 100. - 7.);
        assert_eq!(area.bbox.bottom(), text_top_left.y + text_size.y);
        assert_eq!(area.bbox.width(), text_size.x.max(14.));
    }

    #[test]
    fn an_icon_keeps_other_labels_off_it() {
        let ctx = context();
        let icon = Text::new(pos2(100., 100.), String::new(), 12., Color32::BLACK, 0.)
            .with_icon(Some(peak()));
        let label = Text::new(pos2(100., 100.), "Ślęża".into(), 12., Color32::BLACK, 0.);

        let shapes = place_texts(vec![icon, label], &ctx);
        assert!(!matches!(shapes[0], Shape::Noop));
        assert!(matches!(shapes[1], Shape::Noop));
    }

    fn rect(x: f32, y: f32, w: f32, h: f32, angle: f32) -> OrientedRect {
        OrientedRect::new(pos2(x, y), angle, vec2(w, h))
    }

    #[test]
    fn overlapping_rectangles_intersect() {
        assert!(rect(0., 0., 10., 10., 0.).intersects(&rect(8., 3., 10., 10., 0.)));
        assert!(rect(0., 0., 10., 10., 0.).intersects(&rect(2., 2., 10., 4., 0.7)));
    }

    #[test]
    fn rectangles_apart_do_not_intersect() {
        assert!(!rect(0., 0., 10., 10., 0.).intersects(&rect(20., 0., 10., 10., 0.)));
        assert!(!rect(0., 0., 10., 10., 0.).intersects(&rect(0., -20., 10., 10., 0.3)));
    }

    #[test]
    fn rectangles_sharing_an_edge_intersect() {
        assert!(rect(0., 0., 10., 10., 0.).intersects(&rect(10., 0., 10., 10., 0.)));
    }

    /// The bounding boxes overlap, the rectangles do not: the case the bounding boxes alone
    /// would get wrong.
    #[test]
    fn a_turned_rectangle_off_the_corner_does_not_intersect() {
        let square = rect(0., 0., 10., 10., 0.);
        let diamond = rect(11., 11., 10., 10., std::f32::consts::FRAC_PI_4);

        assert!(square.bbox.intersects(diamond.bbox));
        assert!(!square.intersects(&diamond));
        assert!(!diamond.intersects(&square));
    }

    fn random_rect(rng: &mut StdRng, extent: f32, max_size: f32) -> OrientedRect {
        rect(
            rng.random_range(0. ..extent),
            rng.random_range(0. ..extent),
            rng.random_range(1. ..max_size),
            rng.random_range(1. ..max_size),
            rng.random_range(-std::f32::consts::PI..std::f32::consts::PI),
        )
    }

    fn polygon(rect: &OrientedRect) -> geo::Polygon<f32> {
        let ring: Vec<_> = rect
            .corners
            .iter()
            .chain(&rect.corners[..1])
            .map(|p| geo::Coord { x: p.x, y: p.y })
            .collect();
        geo::Polygon::new(ring.into(), vec![])
    }

    #[test]
    fn intersection_agrees_with_geo() {
        use geo::Intersects;

        let mut rng = StdRng::seed_from_u64(1);
        for _ in 0..20_000 {
            let a = random_rect(&mut rng, 100., 60.);
            let b = random_rect(&mut rng, 100., 60.);
            assert_eq!(
                a.intersects(&b),
                polygon(&a).intersects(&polygon(&b)),
                "{:?} and {:?}",
                a.corners,
                b.corners
            );
        }
    }

    /// The labels the tree takes are those a check against every placed one takes, small and
    /// large ones alike.
    #[test]
    fn placing_agrees_with_checking_every_placed_label() {
        let text = Text::new(pos2(0., 0.), "label".into(), 12., Color32::BLACK, 0.);
        let mut rng = StdRng::seed_from_u64(2);
        let mut placed = PlacedTexts::default();
        let mut reference: Vec<OrientedRect> = Vec::new();

        for k in 0..3000 {
            let max_size = match k % 100 {
                0 => 5000.,
                1..10 => 600.,
                _ => 80.,
            };
            let area = random_rect(&mut rng, 2000., max_size);
            let expected = !reference.iter().any(|r| r.intersects(&area));
            if expected {
                reference.push(area);
            }
            assert_eq!(placed.try_place(&text, area), expected, "label {k}");
        }

        assert!(reference.len() > 100, "the test should place a fair number");
    }

    fn grid_of_labels() -> Vec<Text> {
        (0..400)
            .map(|k| {
                Text::new(
                    pos2((k % 20) as f32 * 30., (k / 20) as f32 * 8.),
                    format!("Label {k}"),
                    12.,
                    Color32::BLACK,
                    0.,
                )
            })
            .collect()
    }

    /// An empty label on a line of no length, whose slope, and so angle, is 0 / 0.
    #[test]
    fn a_label_with_no_angle_is_dropped_and_the_others_placed_as_without_it() {
        let ctx = context();
        let no_angle = Text::new(pos2(0., 0.), String::new(), 12., Color32::BLACK, f32::NAN)
            .with_placement(Placement::Line)
            .with_max_width(Some(0.));
        let mut texts = vec![no_angle];
        texts.extend(grid_of_labels());

        let shapes = place_texts(texts, &ctx);

        assert!(matches!(shapes[0], Shape::Noop));
        assert_eq!(
            shapes[1..]
                .iter()
                .filter(|s| !matches!(s, Shape::Noop))
                .count(),
            count_placed(grid_of_labels(), &ctx)
        );
    }
}
