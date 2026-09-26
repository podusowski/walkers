//! Renderer for Mapbox Vector Tiles.

use std::collections::HashMap;

use ecolor::Color32;
use emath::TSTransform;
use log::warn;
use mvt_reader::{Reader, feature::Value};
use serde_json::{Number, Value as JsonValue};

use geo::MapCoordsInPlace;

use crate::{
    Drawable,
    expression::Context,
    render::{self, Coord, Geometry},
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
pub fn render(
    data: &[u8],
    style: &Style,
    zoom: u8,
    tile_size: u32,
) -> Result<(Vec<Drawable>, Vec<Text>), Error> {
    let tile_layers = decode_needed_layers(data, style, zoom, tile_size)?;
    let mut drawables = Vec::new();
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

                drawables.push(Drawable::background(tile_size as f32, bg_color));
            }
            Layer::Fill {
                source_layer,
                filter,
                paint,
            } => {
                for (geometry, context) in features(&tile_layers, source_layer, filter.as_ref()) {
                    if let Err(err) = render::fill::render(geometry, context, paint, &mut drawables)
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
                for (geometry, context) in features(&tile_layers, source_layer, filter.as_ref()) {
                    if let Err(err) = render::line::render(geometry, context, paint, &mut drawables)
                    {
                        warn!("{err}");
                    }
                }
            }
            Layer::Symbol {
                source_layer,
                minzoom,
                filter,
                layout,
                paint,
            } => {
                for (geometry, context) in features(&tile_layers, source_layer, filter.as_ref()) {
                    if let Err(err) = render::symbol::render(
                        geometry, context, &mut texts, layout, paint, *minzoom,
                    ) {
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

    log::trace!("Rendered {} drawables", drawables.len());
    Ok((render::merge_runs(drawables), texts))
}

/// What takes a tile rendered for `tile_size` onto the `rect` it is actually drawn at.
pub fn transform_onto(rect: egui::Rect, tile_size: u32) -> TSTransform {
    TSTransform {
        scaling: rect.width() / tile_size as f32,
        translation: rect.min.to_vec2(),
    }
}

/// A layer of the tile, with its features decoded only if some style layer draws from it.
struct TileLayer {
    name: String,
    features: Vec<(Geometry<f32>, Context)>,
}

/// Decodes each tile layer at most once, however many style layers draw from it.
fn decode_needed_layers(
    data: &[u8],
    style: &Style,
    zoom: u8,
    tile_size: u32,
) -> Result<Vec<TileLayer>, Error> {
    let reader = Reader::new(data.to_vec())?;
    let into_pixels = tile_size as f32 / ONLY_SUPPORTED_EXTENT as f32;

    let needed = |name: &str| {
        style.layers.iter().any(|layer| match layer {
            Layer::Fill { source_layer, .. }
            | Layer::Line { source_layer, .. }
            | Layer::Symbol { source_layer, .. } => source_layer.matches(name),
            // Circle layers name a source layer, but are not drawn.
            Layer::Background { .. }
            | Layer::Circle { .. }
            | Layer::Raster
            | Layer::FillExtrusion => false,
        })
    };

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
            TileLayer {
                name: layer.name,
                features,
            }
        })
        .collect())
}

/// Features a style layer draws, in the order of the tile's layers.
fn features<'a>(
    tile_layers: &'a [TileLayer],
    source_layer: &'a SourceLayer,
    filter: Option<&'a Filter>,
) -> impl Iterator<Item = &'a (Geometry<f32>, Context)> {
    tile_layers
        .iter()
        .filter(|layer| source_layer.matches(&layer.name))
        .flat_map(|layer| &layer.features)
        .filter(move |(_, context)| filter.is_none_or(|filter| filter.matches(context)))
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
