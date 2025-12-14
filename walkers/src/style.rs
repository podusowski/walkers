use std::collections::HashMap;

use color::Rgba8;
use egui::Color32;
use log::warn;
use mvt_reader::feature::Value as MvtValue;
use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

use crate::expression::evaluate;

/// Style for rendering vector maps.
///
/// It is beased on MapLibre's style specification, but only a small subset is supported.
/// Most notably, Walkers only read `layers` section of the style and applies it to the
/// [`Tiles`] it is used with. In spite that, it should be possible to deserialize most
/// of the MapLibre's styles using `serde`, as unknown JSON/YAML fields are simply ignored.
///
/// https://maplibre.org/maplibre-style-spec/
#[derive(Deserialize, Default)]
pub struct Style {
    pub layers: Vec<Layer>,
}

impl Style {
    pub fn protonmaps_dark() -> Self {
        let style_json = include_str!("../assets/protomaps-dark.json");
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
    },
    Raster,
    FillExtrusion,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct Paint {
    pub fill_color: Option<Color>,
    /// https://maplibre.org/maplibre-style-spec/layers/#fill-opacity
    pub fill_opacity: Option<Opacity>,
}

#[derive(Debug, Error)]
enum ColorError {
    #[error(transparent)]
    Expression(#[from] crate::expression::Error),
    #[error("color must be a string")]
    InvalidType,
    #[error(transparent)]
    Parsing(#[from] color::ParseError),
}

#[derive(Deserialize, Debug)]
pub struct Color(pub Value);

impl Color {
    pub fn evaluate(&self, properties: &HashMap<String, MvtValue>, zoom: u8) -> Color32 {
        match self.try_evaluate(properties, zoom) {
            Ok(color) => color,
            Err(err) => {
                warn!("{:?}", err);
                Color32::MAGENTA
            }
        }
    }

    fn try_evaluate(
        &self,
        properties: &HashMap<String, MvtValue>,
        zoom: u8,
    ) -> Result<Color32, ColorError> {
        match evaluate(&self.0, properties, zoom)? {
            Value::String(color) => {
                let color: color::AlphaColor<color::Srgb> = color.parse()?;
                let Rgba8 { r, g, b, a } = color.to_rgba8();
                Ok(Color32::from_rgba_premultiplied(r, g, b, a))
            }
            _ => Err(ColorError::InvalidType),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct Opacity(Value);

impl Opacity {
    pub fn evaluate(&self, properties: &HashMap<String, MvtValue>, zoom: u8) -> f32 {
        let value = evaluate(&self.0, properties, zoom);

        match value {
            Ok(Value::Number(num)) => num.as_f64().unwrap() as f32,
            other => {
                warn!("Opacity did not evaluate to a number: {:?}", other);
                0.5
            }
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct Filter(pub Value);

impl Filter {
    /// Match this filter against feature properties.
    pub fn matches(&self, properties: &HashMap<String, MvtValue>, zoom: u8) -> bool {
        match evaluate(&self.0, properties, zoom) {
            Ok(Value::Bool(b)) => b,
            other => {
                warn!("Filter did not evaluate to a boolean: {:?}", other);
                false
            }
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct Layout {
    text_field: Option<Value>,
}

impl Layout {
    pub fn text(&self, properties: &HashMap<String, MvtValue>, zoom: u8) -> Option<String> {
        match &self.text_field {
            Some(value) => match evaluate(value, properties, zoom) {
                Ok(Value::String(s)) => Some(s),
                other => {
                    warn!("text-field did not evaluate to a string: {:?}", other);
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
        Style::protonmaps_dark();
    }
}
