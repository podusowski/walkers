//! Renderer for Mapbox Vector Tiles.

use std::collections::HashMap;

use egui::{
    Color32, Mesh, Pos2, Shape, Stroke,
    emath::TSTransform,
    epaint::{Vertex, WHITE_UV},
    pos2,
};
use geo_types::Geometry;
use log::warn;
use lyon_path::{
    Path, Polygon,
    geom::{Point, point},
};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, TessellationError, VertexBuffers,
};
use mvt_reader::feature::{Feature, Value};

use crate::style::{Filter, Layer, Layout, Paint, Style};

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
    Text {
        position: Pos2,
        text: String,
        font_size: f32,
        text_color: Color32,
    },
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
            ShapeOrText::Text { position, .. } => {
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
            Layer::Background => continue,
            Layer::Fill {
                source_layer,
                filter,
                paint,
            } => {
                let Ok(layer_index) = find_layer(&data, source_layer) else {
                    warn!("Source layer '{source_layer}' not found. Skipping.");
                    continue;
                };

                for feature in data.get_features(layer_index)? {
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
                let Ok(layer_index) = find_layer(&data, source_layer) else {
                    warn!("Source layer '{source_layer}' not found. Skipping.");
                    continue;
                };

                for feature in data.get_features(layer_index)? {
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
                let Ok(layer_index) = find_layer(&data, source_layer) else {
                    warn!("Source layer '{source_layer}' not found. Skipping.");
                    continue;
                };

                for feature in data.get_features(layer_index)? {
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

fn match_filter(feature: &Feature, type_: &str, zoom: u8, filter: &Option<Filter>) -> bool {
    // Special property "$type" to filter by geometry type. MapLibre Style spec mentions
    // 'geometry-type', but Protomaps uses '$type' in their styles.
    let properties = feature.properties.clone().map(|mut properties| {
        properties.insert("$type".to_string(), Value::String(type_.to_string()));
        properties
    });
    match (&properties, filter) {
        (Some(properties), Some(filter)) => filter.matches(properties, zoom),
        _ => true,
    }
}

fn line_feature_into_shape(
    feature: &Feature,
    shapes: &mut Vec<ShapeOrText>,
    filter: &Option<Filter>,
    paint: &Paint,
    zoom: u8,
) -> Result<(), Error> {
    if !match_filter(feature, "Line", zoom, filter) {
        return Ok(());
    }

    let properties = feature
        .properties
        .as_ref()
        .ok_or(Error::FeatureWithoutProperties)?;

    let width = if let Some(width) = &paint.line_width {
        // Align to the proportion of MVT extent and tile size.
        width.evaluate(properties, zoom) * 4.0
    } else {
        2.0
    };

    let opacity = if let Some(opacity) = &paint.line_opacity {
        opacity.evaluate(properties, zoom)
    } else {
        1.0
    };

    let color = if let Some(color) = &paint.line_color {
        color.evaluate(properties, zoom).gamma_multiply(opacity)
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
    if let Geometry::MultiPolygon(multi_polygon) = &feature.geometry {
        if !match_filter(feature, "Polygon", zoom, filter) {
            return Ok(());
        }

        let Some(fill_color) = &paint.fill_color else {
            warn!("Fill layer without fill color. Skipping.");
            return Ok(());
        };

        let fill_color = fill_color.evaluate(properties, zoom);

        let fill_color = if let Some(fill_opacity) = &paint.fill_opacity {
            let fill_opacity = fill_opacity.evaluate(properties, zoom);
            fill_color.gamma_multiply(fill_opacity)
        } else {
            fill_color
        };

        for polygon in multi_polygon.iter() {
            let points = polygon
                .exterior()
                .0
                .iter()
                .map(|p| pos2(p.x, p.y))
                .collect::<Vec<_>>();
            let interiors = polygon
                .interiors()
                .iter()
                .map(|hole| hole.0.iter().map(|p| pos2(p.x, p.y)).collect::<Vec<_>>())
                .collect::<Vec<_>>();
            shapes.push(tessellate_polygon(&points, &interiors, fill_color)?.into());
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
    if let Geometry::MultiPoint(multi_point) = &feature.geometry {
        if !match_filter(feature, "Point", zoom, filter) {
            return Ok(());
        }

        let text_size = layout
            .text_size
            .as_ref()
            .and_then(|text_size| {
                let size = text_size.evaluate(properties, zoom);

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
            color.evaluate(properties, zoom)
        } else {
            Color32::BLACK
        };

        if let Some(text) = &layout.text(properties, zoom) {
            shapes.extend(multi_point.0.iter().map(|p| ShapeOrText::Text {
                position: pos2(p.x(), p.y()),
                text: text.clone(),
                font_size: text_size,
                text_color,
            }))
        }
    }

    if let Geometry::MultiLineString(multi_line_string) = &feature.geometry {
        if !match_filter(feature, "Point", zoom, filter) {
            return Ok(());
        }

        let text_size = layout
            .text_size
            .as_ref()
            .and_then(|text_size| {
                let size = text_size.evaluate(properties, zoom);

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
            color.evaluate(properties, zoom)
        } else {
            Color32::BLACK
        };

        for line_string in multi_line_string {
            let mid_index = line_string.0.len() / 2;
            let mid_point = &line_string.0[mid_index];

            if let Some(text) = &layout.text(properties, zoom) {
                shapes.push(ShapeOrText::Text {
                    position: pos2(mid_point.x, mid_point.y),
                    text: text.clone(),
                    font_size: text_size,
                    text_color,
                });
            }
        }
    }
    Ok(())
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
fn tessellate_polygon(
    exterior: &[Pos2],
    interiors: &[Vec<Pos2>],
    fill_color: Color32,
) -> Result<Mesh, TessellationError> {
    let mut builder = Path::builder();

    builder.add_polygon(Polygon {
        points: &lyon_points(exterior),
        closed: true,
    });

    for interior in interiors {
        builder.add_polygon(Polygon {
            points: &lyon_points(interior),
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

fn lyon_points(points: &[Pos2]) -> Vec<Point<f32>> {
    points.iter().map(|p| point(p.x, p.y)).collect()
}
