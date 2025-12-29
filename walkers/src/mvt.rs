//! Renderer for Mapbox Vector Tiles.

use std::collections::HashMap;

use egui::{
    Color32, Mesh, Pos2, Rect, Shape, Stroke, Vec2,
    emath::TSTransform,
    epaint::{Vertex, WHITE_UV},
    pos2, vec2,
};
use geo_types::{Coord, Geometry, Line};
use log::warn;
use lyon_path::{
    Path, Polygon,
    geom::{Point, point},
};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, TessellationError, VertexBuffers,
};
use mvt_reader::{
    Reader,
    feature::{Feature, Value},
};

use crate::{
    expression::Context,
    style::{Filter, Layer, Layout, Paint, Style},
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
    pub corners: [egui::Pos2; 4],
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

/// Render MVT data into a list of [`epaint::Shape`]s.
pub fn render(data: &[u8], style: &Style, zoom: u8) -> Result<Vec<ShapeOrText>, Error> {
    let data = mvt_reader::Reader::new(data.to_vec())?;
    let mut shapes = Vec::new();

    for layer in &style.layers {
        match layer {
            Layer::Background { paint } => {
                let properties = HashMap::new();
                let context = Context::new("Polygon".to_string(), &properties, zoom);

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
                for feature in get_layer_features(&data, source_layer)? {
                    if let Err(err) =
                        polygon_feature_into_shape(&feature, &mut shapes, filter, paint, zoom)
                    {
                        warn!("{err}");
                    }
                }
            }
            Layer::Line {
                source_layer,
                filter,
                paint,
            } => {
                for feature in get_layer_features(&data, source_layer)? {
                    if let Err(err) =
                        line_feature_into_shape(&feature, &mut shapes, filter, paint, zoom)
                    {
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
                for feature in get_layer_features(&data, source_layer)? {
                    if let Err(err) =
                        symbol_into_shape(&feature, &mut shapes, filter, layout, paint, zoom)
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

fn get_layer_features(reader: &Reader, name: &str) -> Result<Vec<Feature>, Error> {
    if let Ok(layer_index) = find_layer(reader, name) {
        Ok(reader.get_features(layer_index)?)
    } else {
        warn!("Source layer '{name}' not found. Skipping.");
        Ok(Vec::new())
    }
}

fn line_feature_into_shape(
    feature: &Feature,
    shapes: &mut Vec<ShapeOrText>,
    filter: &Option<Filter>,
    paint: &Paint,
    zoom: u8,
) -> Result<(), Error> {
    let properties = feature
        .properties
        .as_ref()
        .ok_or(Error::FeatureWithoutProperties)?;

    let context = Context::new("Line".to_string(), properties, zoom);

    if let Some(filter) = filter
        && !filter.matches(&context)
    {
        return Ok(());
    }

    let width = if let Some(width) = &paint.line_width {
        // Align to the proportion of MVT extent and tile size.
        width.evaluate(&context) * 4.0
    } else {
        2.0
    };

    let opacity = if let Some(opacity) = &paint.line_opacity {
        opacity.evaluate(&context)
    } else {
        1.0
    };

    let color = if let Some(color) = &paint.line_color {
        color.evaluate(&context).gamma_multiply(opacity)
    } else {
        Color32::WHITE
    };

    match &feature.geometry {
        Geometry::LineString(line_string) => {
            let stroke = Stroke::new(width, color);
            let points = line_string
                .0
                .iter()
                .map(|p| pos2(p.x, p.y))
                .collect::<Vec<_>>();
            shapes.push(Shape::line(points, stroke).into());
        }
        Geometry::MultiLineString(multi_line_string) => {
            let stroke = Stroke::new(width, color);
            for line_string in multi_line_string {
                let points = line_string
                    .0
                    .iter()
                    .map(|p| pos2(p.x, p.y))
                    .collect::<Vec<_>>();
                shapes.push(Shape::line(points, stroke).into());
            }
        }
        _ => (),
    }
    Ok(())
}

fn polygon_feature_into_shape(
    feature: &Feature,
    shapes: &mut Vec<ShapeOrText>,
    filter: &Option<Filter>,
    paint: &Paint,
    zoom: u8,
) -> Result<(), Error> {
    let properties = feature
        .properties
        .as_ref()
        .ok_or(Error::FeatureWithoutProperties)?;

    let context = Context::new("Polygon".to_string(), properties, zoom);

    if let Geometry::MultiPolygon(multi_polygon) = &feature.geometry {
        if let Some(filter) = filter
            && !filter.matches(&context)
        {
            return Ok(());
        }

        let Some(fill_color) = &paint.fill_color else {
            warn!("Fill layer without fill color. Skipping.");
            return Ok(());
        };

        let fill_color = fill_color.evaluate(&context);

        let fill_color = if let Some(fill_opacity) = &paint.fill_opacity {
            let fill_opacity = fill_opacity.evaluate(&context);
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

/// Render a shape from symbol layer.
fn symbol_into_shape(
    feature: &Feature,
    shapes: &mut Vec<ShapeOrText>,
    filter: &Option<Filter>,
    layout: &Layout,
    paint: &Option<Paint>,
    zoom: u8,
) -> Result<(), Error> {
    let properties = feature
        .properties
        .as_ref()
        .ok_or(Error::FeatureWithoutProperties)?;

    let context = Context::new("Point".to_string(), properties, zoom);

    match &feature.geometry {
        Geometry::MultiPoint(multi_point) => {
            if let Some(filter) = filter
                && !filter.matches(&context)
            {
                return Ok(());
            }

            let text_size = layout
                .text_size
                .as_ref()
                .and_then(|text_size| {
                    let size = text_size.evaluate(&context);

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
                .unwrap_or(12.0);

            let text_color = if let Some(paint) = paint
                && let Some(color) = &paint.text_color
            {
                color.evaluate(&context)
            } else {
                // Default from MapLibre spec.
                Color32::BLACK
            };

            if let Some(text) = &layout.text(&context) {
                shapes.extend(multi_point.0.iter().map(|p| {
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
        }
        Geometry::MultiLineString(multi_line_string) => {
            if let Some(filter) = filter
                && !filter.matches(&context)
            {
                return Ok(());
            }

            let text_size = layout
                .text_size
                .as_ref()
                .and_then(|text_size| {
                    let size = text_size.evaluate(&context);

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
                .unwrap_or(12.0);

            let text_color = if let Some(paint) = paint
                && let Some(color) = &paint.text_color
            {
                color.evaluate(&context)
            } else {
                Color32::BLACK
            };

            let text_halo_color = if let Some(paint) = paint
                && let Some(color) = &paint.text_halo_color
            {
                color.evaluate(&context)
            } else {
                Color32::TRANSPARENT
            };

            for line_string in multi_line_string {
                let lines: Vec<_> = line_string.lines().collect();

                if let Some(text) = &layout.text(&context)
                // Use the longest line to fit the label.
                && let Some(line) = lines.into_iter().max_by_key(|line| length(line) as u32)
                {
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
        _ => (),
    }
    Ok(())
}

fn length(line: &Line<f32>) -> f32 {
    (line.dx() * line.dx() + line.dy() * line.dy()).sqrt()
}

fn midpoint(p1: &geo_types::Point<f32>, p2: &geo_types::Point<f32>) -> geo_types::Point<f32> {
    geo_types::Point::new((p1.x() + p2.x()) / 2.0, (p1.y() + p2.y()) / 2.0)
}

fn find_layer(data: &mvt_reader::Reader, name: &str) -> Result<usize, Error> {
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
