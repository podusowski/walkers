//! Renderer for Mapbox Vector Tiles.

use std::collections::HashMap;

use egui::{Color32, Rect, Shape, emath::TSTransform, pos2, vec2};
use log::warn;
use mvt_reader::{Reader, feature::Value};
use serde_json::{Number, Value as JsonValue};

use crate::{
    expression::Context,
    render::{self, Geometry, ShapeOrText},
    style::{Filter, Layer, Style},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Decoding MVT failed: {0}.")]
    Mvt(String),
    #[error("Layer not found: {0}. Available layers: {1:?}")]
    LayerNotFound(String, Vec<String>),
    #[error("Unsupported layer extent: {0}")]
    UnsupportedLayerExtent(String),
}

/// Custom conversion because mvt_reader::error::Error is not Send.
impl From<mvt_reader::error::ParserError> for Error {
    fn from(err: mvt_reader::error::ParserError) -> Self {
        Error::Mvt(err.to_string())
    }
}

/// Currently this is the only supported extent.
const ONLY_SUPPORTED_EXTENT: u32 = 4096;

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
                    if let Err(err) =
                        render::render_polygon(&geometry, &context, &mut shapes, paint)
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
                for (geometry, context) in
                    get_layer_features(&data, zoom, source_layer, filter.as_ref())?
                {
                    if let Err(err) = render::render_line(&geometry, &context, &mut shapes, paint) {
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
                    if let Err(err) =
                        render::render_symbol(&geometry, &context, &mut shapes, layout, paint)
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
            render::geometry_type_to_str(&feature.geometry).to_string(),
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
