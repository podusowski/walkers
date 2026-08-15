//! Renderer for Mapbox Vector Tiles.

use std::collections::HashMap;

use egui::{Color32, Rect, Shape, emath::TSTransform, pos2, vec2};
use log::warn;
use mvt_reader::{Reader, feature::Value};
use serde_json::{Number, Value as JsonValue};

use crate::{
    expression::Context,
    render::{self, Geometry},
    style::{Filter, Layer, SourceLayer, Style},
    text::Text,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Decoding MVT failed: {0}.")]
    Mvt(String),
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
pub fn render(data: &[u8], style: &Style, zoom: u8) -> Result<(Vec<Shape>, Vec<Text>), Error> {
    let data = mvt_reader::Reader::new(data.to_vec())?;
    let mut shapes = Vec::new();
    let mut texts = Vec::new();

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
                shapes.push(Shape::rect_filled(rect, 0.0, bg_color));
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
                        render::render_symbol(&geometry, &context, &mut texts, layout, paint)
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
    Ok((shapes, texts))
}

/// What takes a tile from MVT space onto the screen.
pub fn transform_onto(rect: egui::Rect) -> TSTransform {
    TSTransform {
        scaling: rect.width() / ONLY_SUPPORTED_EXTENT as f32,
        translation: rect.min.to_vec2(),
    }
}

fn get_layer_features(
    reader: &Reader,
    zoom: u8,
    source_layer: &SourceLayer,
    filter: Option<&Filter>,
) -> Result<impl Iterator<Item = (Geometry<f32>, Context)>, Error> {
    let mut matched = 0usize;
    let mut raw = Vec::new();

    for layer in reader.get_layer_metadata()? {
        if !source_layer.matches(&layer.name) {
            continue;
        }

        matched += 1;

        if layer.extent != ONLY_SUPPORTED_EXTENT {
            warn!(
                "Unsupported extent in source layer '{}'. Skipping.",
                layer.name
            );
            continue;
        }

        raw.extend(reader.get_features(layer.layer_index).unwrap_or_default());
    }

    // Asking for everything and finding nothing is a tile without features, but asking for a
    // layer by name and not finding it usually means the style does not fit the schema.
    if matched == 0 && !source_layer.is_all() {
        warn!("Source layer {source_layer} not found. Skipping.");
    }

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
