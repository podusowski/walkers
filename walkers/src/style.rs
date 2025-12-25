use std::collections::HashMap;

use color::Rgba8;
use egui::Color32;
use log::warn;
use mvt_reader::feature::Value as MvtValue;
use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

use crate::expression::Context;

/// Style for rendering vector maps.
///
/// It is beased on MapLibre's style specification, but only a small subset is supported.
/// Most notably, Walkers only read `layers` section of the style and applies it to the
/// [`crate::Tiles`] it is used with. In spite that, it should be possible to deserialize most
/// of the MapLibre's styles using `serde`, as unknown JSON/YAML fields are simply ignored.
///
/// <https://maplibre.org/maplibre-style-spec/>
#[derive(Deserialize, Default)]
pub struct Style {
    pub layers: Vec<Layer>,
}

impl Style {
    /// Style based on Protomaps Dark flavour. Requires Protomaps source.
    ///
    /// <https://docs.protomaps.com/basemaps/flavors>
    pub fn protomaps_dark() -> Self {
        let style_json = include_str!("../assets/protomaps-dark.json");
        serde_json::from_str(style_json).expect("failed to parse style JSON")
    }

    /// Style based on Protomaps Dark Vis flavour. Requires Protomaps source.
    ///
    /// <https://docs.protomaps.com/basemaps/flavors>
    pub fn protomaps_dark_vis() -> Self {
        let style_json = include_str!("../assets/protomaps-dark-vis.json");
        serde_json::from_str(style_json).expect("failed to parse style JSON")
    }

    /// Style based on Protomaps Light flavour. Requires Protomaps source.
    ///
    /// <https://docs.protomaps.com/basemaps/flavors>
    pub fn protomaps_light() -> Self {
        let style_json = include_str!("../assets/protomaps-light.json");
        serde_json::from_str(style_json).expect("failed to parse style JSON")
    }
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Layer {
    Background,
    #[serde(rename_all = "kebab-case")]
    Fill {
        source_layer: String,
        filter: Option<Filter>,
        paint: Paint,
    },
    #[serde(rename_all = "kebab-case")]
    Line {
        source_layer: String,
        filter: Option<Filter>,
        paint: Paint,
    },
    #[serde(rename_all = "kebab-case")]
    Symbol {
        source_layer: String,
        filter: Option<Filter>,
        layout: Layout,
        paint: Option<Paint>,
    },
    Raster,
    FillExtrusion,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct Paint {
    pub fill_color: Option<Color>,
    /// https://maplibre.org/maplibre-style-spec/layers/#fill-opacity
    pub fill_opacity: Option<Float>,
    pub line_width: Option<Float>,
    /// https://maplibre.org/maplibre-style-spec/layers/#line-color
    pub line_color: Option<Color>,
    /// https://maplibre.org/maplibre-style-spec/layers/#line-opacity
    pub line_opacity: Option<Float>,
    /// https://maplibre.org/maplibre-style-spec/layers/#text-color
    pub text_color: Option<Color>,
    /// https://maplibre.org/maplibre-style-spec/layers/#text-halo-color
    pub text_halo_color: Option<Color>,
}

#[derive(Debug, Error)]
enum StyleError {
    #[error(transparent)]
    Expression(#[from] crate::expression::Error),
    #[error("invalid type")]
    InvalidType,
    #[error(transparent)]
    Parsing(#[from] color::ParseError),
}

#[derive(Deserialize, Debug)]
pub struct Color(pub Value);

impl Color {
    pub fn evaluate(&self, context: &Context) -> Color32 {
        match self.try_evaluate(context) {
            Ok(color) => color,
            Err(err) => {
                warn!("{err}");
                Color32::MAGENTA
            }
        }
    }

    fn try_evaluate(&self, context: &Context) -> Result<Color32, StyleError> {
        match context.evaluate(&self.0)? {
            Value::String(color) => {
                let color: color::AlphaColor<color::Srgb> = color.parse()?;
                let Rgba8 { r, g, b, a } = color.to_rgba8();
                Ok(Color32::from_rgba_premultiplied(r, g, b, a))
            }
            _ => Err(StyleError::InvalidType),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct Float(pub Value);

impl Float {
    pub fn evaluate(&self, context: &Context) -> f32 {
        match self.try_evaluate(context) {
            Ok(opacity) => opacity,
            Err(err) => {
                warn!("{err}");
                0.5
            }
        }
    }

    fn try_evaluate(&self, context: &Context) -> Result<f32, StyleError> {
        match context.evaluate(&self.0)? {
            Value::Number(num) => Ok(num.as_f64().ok_or(StyleError::InvalidType)? as f32),
            _ => Err(StyleError::InvalidType),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct Filter(pub Value);

impl Filter {
    /// Match this filter against feature properties.
    pub fn matches(&self, properties: &HashMap<String, MvtValue>, zoom: u8) -> bool {
        match Context::new(properties, zoom).evaluate(&self.0) {
            Ok(Value::Bool(b)) => b,
            other => {
                warn!("Expected filter to evaluate to boolean, got: {other:?}");
                false
            }
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct Layout {
    text_field: Option<Value>,
    pub text_size: Option<Float>,
}

impl Layout {
    pub fn text(&self, properties: &HashMap<String, MvtValue>, zoom: u8) -> Option<String> {
        match &self.text_field {
            Some(value) => match Context::new(properties, zoom).evaluate(value) {
                Ok(Value::String(s)) => Some(s),
                other => {
                    warn!("Expected text-field to evaluate to a string, got: {other:?}");
                    None
                }
            },
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_style_parsing() {
        Style::protomaps_dark();
        Style::protomaps_light();
    }
}
