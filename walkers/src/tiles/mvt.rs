//! Decoding Mapbox Vector Tiles.

use std::collections::HashMap;

use log::warn;
use mvt_reader::{Reader, feature::Value};
use serde_json::{Number, Value as JsonValue};

use geo::MapCoordsInPlace;

use super::apply_style::FeatureLayer;
use crate::{
    expression::Context,
    render::{self, Coord},
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

/// Layers of the MVT `data`, in pixels of a tile `tile_size` wide. Only the `needed` ones have
/// their features decoded.
pub fn decode(
    data: &[u8],
    needed: impl Fn(&str) -> bool,
    zoom: u8,
    tile_size: u32,
) -> Result<Vec<FeatureLayer>, Error> {
    let reader = Reader::new(data.to_vec())?;
    let into_pixels = tile_size as f32 / ONLY_SUPPORTED_EXTENT as f32;

    Ok(reader
        .get_layer_metadata()?
        .into_iter()
        .map(|layer| {
            let features = if !needed(&layer.name) {
                Vec::new()
            } else if layer.extent != ONLY_SUPPORTED_EXTENT {
                warn!(
                    "Unsupported extent in source layer '{}'. Skipping.",
                    layer.name
                );
                Vec::new()
            } else {
                reader
                    .get_features(layer.layer_index)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|mut feature| {
                        let context = Context::new(
                            render::geometry_type_to_str(&feature.geometry).to_string(),
                            feature
                                .properties
                                .map_or(Default::default(), mvt_properties_to_json_properties),
                            zoom,
                        );
                        feature.geometry.map_coords_in_place(|coord| Coord {
                            x: coord.x * into_pixels,
                            y: coord.y * into_pixels,
                        });
                        (feature.geometry, context)
                    })
                    .collect()
            };
            FeatureLayer {
                name: layer.name,
                features,
            }
        })
        .collect())
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
