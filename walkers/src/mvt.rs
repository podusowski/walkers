//! Renderer for Mapbox Vector Tiles.

use std::collections::HashMap;

use egui::{
    Color32, Mesh, Rect, Shape, Stroke,
    emath::TSTransform,
    epaint::{Vertex, WHITE_UV},
    pos2, vec2,
};
pub use geo_types::{Coord, Geometry, Line};
use log::warn;
use lyon_path::{
    Path, Polygon,
    geom::{Point, point},
};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, TessellationError, VertexBuffers,
};
use mvt_reader::{Reader, feature::Value};
use serde_json::{Number, Value as JsonValue};

use crate::{
    expression::Context,
    style::{Filter, Layer, Layout, Paint, Style},
    text::{OccupiedAreas, OrientedRect, Text},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Decoding MVT failed: {0}.")]
    Mvt(String),
    #[error("Layer not found: {0}. Available layers: {1:?}")]
    LayerNotFound(String, Vec<String>),
    #[error("Unsupported layer extent: {0}")]
    UnsupportedLayerExtent(String),
    #[error("Unsupported kind: {0:?}")]
    UnsupportedFeatureKind(HashMap<String, Value>),
    #[error("Missing kind in properties: {0:?}")]
    FeatureWithoutKind(HashMap<String, Value>),
    #[error("Missing properties in feature")]
    FeatureWithoutProperties,
    #[error(transparent)]
    Tessellation(#[from] TessellationError),
}

/// Custom conversion because mvt_reader::error::Error is not Send.
impl From<mvt_reader::error::ParserError> for Error {
    fn from(err: mvt_reader::error::ParserError) -> Self {
        Error::Mvt(err.to_string())
    }
}

/// Currently this is the only supported extent.
const ONLY_SUPPORTED_EXTENT: u32 = 4096;

#[derive(Debug, Clone)]
pub enum ShapeOrText {
    Shape(Shape),
    Text(Text),
}

impl From<Shape> for ShapeOrText {
    fn from(shape: Shape) -> Self {
        ShapeOrText::Shape(shape)
    }
}

impl From<Mesh> for ShapeOrText {
    fn from(mesh: Mesh) -> Self {
        ShapeOrText::Shape(Shape::Mesh(mesh.into()))
    }
}

impl ShapeOrText {
    pub fn transform(&mut self, transform: TSTransform) {
        match self {
            ShapeOrText::Shape(shape) => {
                shape.transform(transform);
            }
            ShapeOrText::Text(Text { position, .. }) => {
                *position *= transform.scaling;
                *position += transform.translation;
            }
        }
    }
}

/// Render MVT data into a list of [`epaint::Shape`]s.
pub fn render(data: &[u8], style: &Style, zoom: u8) -> Result<Vec<ShapeOrText>, Error> {
    let data = mvt_reader::Reader::new(data.to_vec())?;
    let mut shapes = Vec::new();

    for layer in &style.layers {
        match layer {
            Layer::Background { paint } => {
                let context = Context::new("None".to_string(), HashMap::new(), zoom);

                let bg_color = if let Some(color) = &paint.background_color {
                    color.evaluate(&context)
                } else {
                    Color32::WHITE
                };

                let rect = Rect::from_min_size(
                    pos2(0.0, 0.0),
                    vec2(ONLY_SUPPORTED_EXTENT as f32, ONLY_SUPPORTED_EXTENT as f32),
                );
                shapes.push(Shape::rect_filled(rect, 0.0, bg_color).into());
            }
            Layer::Fill {
                source_layer,
                filter,
                paint,
            } => {
                for (geometry, context) in
                    get_layer_features(&data, zoom, source_layer, filter.as_ref())?
                {
                    if let Err(err) = render_polygon(&geometry, &context, &mut shapes, paint) {
                        warn!("{err}");
                    }
                }
            }
            Layer::Line {
                source_layer,
                filter,
                paint,
            } => {
                for (geometry, context) in
                    get_layer_features(&data, zoom, source_layer, filter.as_ref())?
                {
                    if let Err(err) = render_line(&geometry, &context, &mut shapes, paint) {
                        warn!("{err}");
                    }
                }
            }
            Layer::Symbol {
                source_layer,
                filter,
                layout,
                paint,
            } => {
                for (geometry, context) in
                    get_layer_features(&data, zoom, source_layer, filter.as_ref())?
                {
                    if let Err(err) = render_symbol(&geometry, &context, &mut shapes, layout, paint)
                    {
                        warn!("{err}");
                    }
                }
            }
            layer => {
                log::warn!("Unsupported layer type in style: {layer:?}");
                continue;
            }
        }
    }

    log::trace!("Rendered {} shapes", shapes.len());
    Ok(shapes)
}

/// Transform shapes from MVT space to screen space.
pub fn transformed(shapes: &[ShapeOrText], rect: egui::Rect) -> Vec<ShapeOrText> {
    let transform = TSTransform {
        scaling: rect.width() / ONLY_SUPPORTED_EXTENT as f32,
        translation: rect.min.to_vec2(),
    };

    let mut result = shapes.to_vec();
    for shape in result.iter_mut() {
        shape.transform(transform);
    }
    result
}

/// Lay out the labels, dropping the ones which would land on top of an already placed one.
pub fn resolve_text(shapes: Vec<ShapeOrText>, ctx: &egui::Context) -> Vec<Shape> {
    let mut occupied_text_areas = OccupiedAreas::new();

    // Need to collect it to avoid deadlock caused by `Painter::extend` and `fonts_mut`.
    shapes
        .into_iter()
        .map(|shape_or_text| match shape_or_text {
            ShapeOrText::Shape(shape) => shape,
            ShapeOrText::Text(text) => draw_text(text, ctx, &mut occupied_text_areas),
        })
        .collect()
}

fn draw_text(text: Text, ctx: &egui::Context, occupied_text_areas: &mut OccupiedAreas) -> Shape {
    use egui::epaint::TextShape;

    let mut layout_job = egui::text::LayoutJob::default();

    layout_job.append(
        &text.text,
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::proportional(text.font_size),
            color: text.text_color,
            background: text.background_color,
            ..Default::default()
        },
    );

    let galley = ctx.fonts_mut(|fonts| fonts.layout_job(layout_job));

    let area = OrientedRect::new(text.position, text.angle, galley.size());
    let top_left = area.top_left();

    if occupied_text_areas.try_occupy(area) {
        TextShape::new(top_left, galley, text.text_color)
            .with_angle(text.angle)
            .into()
    } else {
        Shape::Noop
    }
}

fn get_layer_features(
    reader: &Reader,
    zoom: u8,
    name: &str,
    filter: Option<&Filter>,
) -> Result<impl Iterator<Item = (Geometry<f32>, Context)>, Error> {
    // An empty source layer matches features from all layers. Intended for sparse
    // overlay tiles; pointing dense basemap rules at "" would scan every layer.
    let raw = if name.is_empty() {
        reader
            .get_layer_metadata()?
            .into_iter()
            .filter(|layer| layer.extent == ONLY_SUPPORTED_EXTENT)
            .flat_map(|layer| reader.get_features(layer.layer_index).unwrap_or_default())
            .collect()
    } else if let Ok(layer_index) = find_layer(reader, name) {
        reader.get_features(layer_index)?
    } else {
        warn!("Source layer '{name}' not found. Skipping.");
        Vec::new()
    };

    let features = raw.into_iter().filter_map(move |feature| {
        let context = Context::new(
            geometry_type_to_str(&feature.geometry).to_string(),
            feature
                .properties
                .map_or(Default::default(), mvt_properties_to_json_properties),
            zoom,
        );

        filter
            .is_none_or(|filter| filter.matches(&context))
            .then_some((feature.geometry, context))
    });

    Ok(features)
}

fn mvt_properties_to_json_properties(
    properties: HashMap<String, mvt_reader::feature::Value>,
) -> HashMap<String, serde_json::Value> {
    properties
        .into_iter()
        .map(|(k, v)| (k, mvt_value_to_json_value(&v)))
        .collect()
}

fn mvt_value_to_json_value(value: &Value) -> JsonValue {
    match value {
        Value::String(s) => JsonValue::String(s.clone()),
        Value::Int(x) | Value::SInt(x) => JsonValue::Number((*x).into()),
        Value::Double(x) => Number::from_f64(*x)
            .map(JsonValue::Number)
            .unwrap_or_else(|| {
                warn!("Invalid f64 value: {x}");
                JsonValue::Null
            }),
        Value::Bool(b) => JsonValue::Bool(*b),
        Value::Null => JsonValue::Null,
        _ => {
            warn!("Unsupported MVT value type: {value:?}");
            JsonValue::Null
        }
    }
}

fn geometry_type_to_str(geometry: &Geometry<f32>) -> &'static str {
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

pub fn render_line(
    geometry: &Geometry<f32>,
    context: &Context,
    shapes: &mut Vec<ShapeOrText>,
    paint: &Paint,
) -> Result<(), Error> {
    let width = if let Some(width) = &paint.line_width {
        // Align to the proportion of MVT extent and tile size.
        width.evaluate(context) * 4.0
    } else {
        2.0
    };

    let opacity = if let Some(opacity) = &paint.line_opacity {
        opacity.evaluate(context)
    } else {
        1.0
    };

    let color = if let Some(color) = &paint.line_color {
        color.evaluate(context).gamma_multiply(opacity)
    } else {
        Color32::WHITE
    };

    let dasharray = paint
        .line_dasharray
        .as_ref()
        .and_then(|dasharray| dasharray.evaluate(context));

    let stroke = Stroke::new(width, color);

    match geometry {
        Geometry::LineString(line_string) => {
            let points = line_string
                .0
                .iter()
                .map(|p| pos2(p.x, p.y))
                .collect::<Vec<_>>();
            push_line(shapes, points, stroke, dasharray.as_deref());
        }
        Geometry::MultiLineString(multi_line_string) => {
            for line_string in multi_line_string {
                let points = line_string
                    .0
                    .iter()
                    .map(|p| pos2(p.x, p.y))
                    .collect::<Vec<_>>();
                push_line(shapes, points, stroke, dasharray.as_deref());
            }
        }
        _ => (),
    }

    Ok(())
}

/// Push a polyline as one or more shapes, splitting it into dashes if `dasharray` is given.
fn push_line(
    shapes: &mut Vec<ShapeOrText>,
    points: Vec<egui::Pos2>,
    stroke: Stroke,
    dasharray: Option<&[f32]>,
) {
    match dasharray {
        Some(pattern) if !pattern.is_empty() => {
            for segment in dash_polyline(&points, pattern, stroke.width) {
                if segment.len() >= 2 {
                    shapes.push(Shape::line(segment, stroke).into());
                }
            }
        }
        _ => shapes.push(Shape::line(points, stroke).into()),
    }
}

/// Split a polyline into the "on" (dash) runs of a dash/gap `pattern`, whose values are
/// in units of `width` per the MapLibre `line-dasharray` spec. Each returned run is a
/// standalone polyline meant to be drawn as its own [`Shape::line`].
fn dash_polyline(points: &[egui::Pos2], pattern: &[f32], width: f32) -> Vec<Vec<egui::Pos2>> {
    let pattern = pattern
        .iter()
        .map(|value| value * width)
        .collect::<Vec<_>>();

    if points.len() < 2 || pattern.iter().sum::<f32>() <= 0.0 {
        return vec![points.to_vec()];
    }

    let mut segments = Vec::new();
    let mut current = vec![points[0]];
    let mut pattern_index = 0;
    let mut remaining = pattern[0];
    let mut drawing = true;

    for window in points.windows(2) {
        let (mut start, end) = (window[0], window[1]);
        let mut length = start.distance(end);

        while length > 0.0 {
            if remaining >= length {
                remaining -= length;
                if drawing {
                    current.push(end);
                }
                length = 0.0;
            } else {
                let point = start + (end - start) * (remaining / length);

                if drawing {
                    current.push(point);
                    segments.push(std::mem::take(&mut current));
                } else {
                    current = vec![point];
                }

                length -= remaining;
                start = point;
                drawing = !drawing;
                pattern_index = (pattern_index + 1) % pattern.len();
                remaining = pattern[pattern_index];
            }
        }
    }

    if drawing && current.len() > 1 {
        segments.push(current);
    }

    segments
}

fn render_polygon(
    geometry: &Geometry<f32>,
    context: &Context,
    shapes: &mut Vec<ShapeOrText>,
    paint: &Paint,
) -> Result<(), Error> {
    if let Geometry::MultiPolygon(multi_polygon) = geometry {
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

        for polygon in multi_polygon.iter() {
            let exterior = lyon_points(&polygon.exterior().0);
            let interiors = polygon
                .interiors()
                .iter()
                .map(|hole| lyon_points(&hole.0))
                .collect::<Vec<_>>();
            shapes.push(tessellate_polygon(&exterior, &interiors, fill_color)?.into());
        }
    }
    Ok(())
}

pub fn render_symbol(
    geometry: &Geometry<f32>,
    context: &Context,
    shapes: &mut Vec<ShapeOrText>,
    layout: &Layout,
    paint: &Option<Paint>,
) -> Result<(), Error> {
    match geometry {
        Geometry::Point(point) => {
            label_points(std::slice::from_ref(point), context, shapes, layout, paint)
        }
        Geometry::MultiPoint(multi_point) => {
            label_points(&multi_point.0, context, shapes, layout, paint)
        }
        Geometry::LineString(line_string) => label_line_strings(
            std::slice::from_ref(line_string),
            context,
            shapes,
            layout,
            paint,
        ),
        Geometry::MultiLineString(multi_line_string) => {
            label_line_strings(&multi_line_string.0, context, shapes, layout, paint)
        }
        _ => (),
    }
    Ok(())
}

fn label_points(
    points: &[geo_types::Point<f32>],
    context: &Context,
    shapes: &mut Vec<ShapeOrText>,
    layout: &Layout,
    paint: &Option<Paint>,
) {
    let Some(text) = layout.text(context) else {
        return;
    };

    let text_size = evaluate_text_size(layout, context);
    let text_color = evaluate_text_color(paint, context);

    shapes.extend(points.iter().map(|p| {
        ShapeOrText::Text(Text::new(
            pos2(p.x(), p.y()),
            text.clone(),
            text_size,
            text_color,
            Color32::TRANSPARENT,
            0.0,
        ))
    }))
}

fn label_line_strings(
    line_strings: &[geo_types::LineString<f32>],
    context: &Context,
    shapes: &mut Vec<ShapeOrText>,
    layout: &Layout,
    paint: &Option<Paint>,
) {
    let Some(text) = layout.text(context) else {
        return;
    };

    let text_size = evaluate_text_size(layout, context);
    let text_color = evaluate_text_color(paint, context);

    let text_halo_color = if let Some(paint) = paint
        && let Some(color) = &paint.text_halo_color
    {
        color.evaluate(context)
    } else {
        Color32::TRANSPARENT
    };

    for line_string in line_strings {
        let lines: Vec<_> = line_string.lines().collect();

        // Use the longest line to fit the label.
        if let Some(line) = lines.into_iter().max_by_key(|line| length(line) as u32) {
            let mid_point = midpoint(&line.start_point(), &line.end_point());
            let angle = line.slope().atan();

            shapes.push(ShapeOrText::Text(Text::new(
                pos2(mid_point.x(), mid_point.y()),
                text.clone(),
                text_size,
                text_color,
                // TODO: Implement real halo rendering.
                text_halo_color.gamma_multiply(0.5),
                angle,
            )));
        }
    }
}

fn evaluate_text_size(layout: &Layout, context: &Context) -> f32 {
    layout
        .text_size
        .as_ref()
        .and_then(|text_size| {
            let size = text_size.evaluate(context);

            if size > 3.0 {
                Some(size)
            } else {
                warn!(
                    "{} evaluated into {size}, which is too small for text size.",
                    text_size.0
                );
                None
            }
        })
        // Default from MapLibre spec.
        .unwrap_or(12.0)
}

fn evaluate_text_color(paint: &Option<Paint>, context: &Context) -> Color32 {
    if let Some(paint) = paint
        && let Some(color) = &paint.text_color
    {
        color.evaluate(context)
    } else {
        // Default from MapLibre spec.
        Color32::BLACK
    }
}

fn length(line: &Line<f32>) -> f32 {
    (line.dx() * line.dx() + line.dy() * line.dy()).sqrt()
}

fn midpoint(p1: &geo_types::Point<f32>, p2: &geo_types::Point<f32>) -> geo_types::Point<f32> {
    geo_types::Point::new((p1.x() + p2.x()) / 2.0, (p1.y() + p2.y()) / 2.0)
}

fn find_layer(data: &Reader, name: &str) -> Result<usize, Error> {
    let layer = data
        .get_layer_metadata()?
        .into_iter()
        .find(|layer| layer.name == name);

    let Some(layer) = layer else {
        return Err(Error::LayerNotFound(
            name.to_string(),
            data.get_layer_names()?,
        ));
    };

    if layer.extent != ONLY_SUPPORTED_EXTENT {
        return Err(Error::UnsupportedLayerExtent(name.to_string()));
    }

    Ok(layer.layer_index)
}

/// Egui cannot tessellate complex polygons, so we use lyon for that.
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
            let pos = vertex.position();
            Vertex {
                pos: pos2(pos.x, pos.y),
                uv: WHITE_UV,
                color: fill_color,
            }
        }),
    )?;

    Ok(Mesh {
        indices: buffers.indices,
        vertices: buffers.vertices,
        ..Default::default()
    })
}

/// Convert list of `geo_types::Coord` to Lyon's `Point`s.
fn lyon_points(points: &[Coord<f32>]) -> Vec<Point<f32>> {
    points.iter().map(|p| point(p.x, p.y)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dash_polyline_without_pattern_keeps_a_single_segment() {
        let points = vec![pos2(0.0, 0.0), pos2(10.0, 0.0)];
        let segments = dash_polyline(&points, &[], 1.0);
        assert_eq!(segments, vec![points]);
    }

    #[test]
    fn dash_polyline_splits_dashes_and_gaps() {
        // Pattern [2, 1] at width 1.0 means a 2-unit dash followed by a 1-unit gap,
        // repeating. Over a 10-unit straight line that's dash/gap/dash/gap/dash/gap...
        let points = vec![pos2(0.0, 0.0), pos2(9.0, 0.0)];
        let segments = dash_polyline(&points, &[2.0, 1.0], 1.0);

        assert_eq!(
            segments,
            vec![
                vec![pos2(0.0, 0.0), pos2(2.0, 0.0)],
                vec![pos2(3.0, 0.0), pos2(5.0, 0.0)],
                vec![pos2(6.0, 0.0), pos2(8.0, 0.0)],
                vec![pos2(9.0, 0.0)],
            ]
            .into_iter()
            .filter(|segment: &Vec<egui::Pos2>| segment.len() >= 2)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn dash_polyline_pattern_scales_with_line_width() {
        let points = vec![pos2(0.0, 0.0), pos2(4.0, 0.0)];
        // width 2.0 turns the [2, 1] pattern into 4-unit dash, 2-unit gap.
        let segments = dash_polyline(&points, &[2.0, 1.0], 2.0);
        assert_eq!(segments, vec![vec![pos2(0.0, 0.0), pos2(4.0, 0.0)]]);
    }

    fn label(geometry: Geometry<f32>) -> Vec<ShapeOrText> {
        let context = Context::new(
            geometry_type_to_str(&geometry).to_string(),
            HashMap::from([("name".to_string(), JsonValue::from("Śnieżka"))]),
            12,
        );

        let layout = Layout {
            text_field: Some(crate::style::json!(["get", "name"])),
            text_size: None,
        };

        let mut shapes = Vec::new();
        render_symbol(&geometry, &context, &mut shapes, &layout, &None).unwrap();
        shapes
    }

    fn texts(shapes: &[ShapeOrText]) -> Vec<&str> {
        shapes
            .iter()
            .filter_map(|shape| match shape {
                ShapeOrText::Text(text) => Some(text.text.as_str()),
                ShapeOrText::Shape(_) => None,
            })
            .collect()
    }

    /// MVT hands over `MultiPoint`, GeoJSON a plain `Point`, and both need labelling.
    #[test]
    fn points_are_labelled_whether_they_come_singly_or_not() {
        let point = geo_types::Point::new(1.0, 2.0);

        assert_eq!(texts(&label(Geometry::Point(point))), ["Śnieżka"]);
        assert_eq!(
            texts(&label(Geometry::MultiPoint(
                vec![point, geo_types::Point::new(3.0, 4.0)].into()
            ))),
            ["Śnieżka", "Śnieżka"]
        );
    }

    #[test]
    fn line_strings_are_labelled_whether_they_come_singly_or_not() {
        let line_string =
            geo_types::LineString::from(vec![(0.0f32, 0.0f32), (10.0, 0.0), (10.0, 10.0)]);

        assert_eq!(
            texts(&label(Geometry::LineString(line_string.clone()))),
            ["Śnieżka"]
        );
        assert_eq!(
            texts(&label(Geometry::MultiLineString(
                geo_types::MultiLineString::new(vec![line_string.clone(), line_string])
            ))),
            ["Śnieżka", "Śnieżka"]
        );
    }

    /// Labels are placed on the geometry, not at the origin.
    #[test]
    fn a_point_label_lands_on_the_point() {
        let shapes = label(Geometry::Point(geo_types::Point::new(1.0, 2.0)));

        match shapes.as_slice() {
            [ShapeOrText::Text(text)] => assert_eq!(text.position, pos2(1.0, 2.0)),
            other => panic!("expected a single label, got {other:?}"),
        }
    }
}
