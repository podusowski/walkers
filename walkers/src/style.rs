use std::collections::HashMap;

use color::Rgba8;
use egui::Color32;
use log::warn;
use mvt_reader::feature::Value as MvtValue;
use serde::Deserialize;
use serde_json::Value;

use crate::expression::evaluate;

/// Style for rendering vector maps. Based on MapLibre's style specification.
/// https://maplibre.org/maplibre-style-spec/
#[derive(Deserialize)]
pub struct Style {
    pub layers: Vec<Layer>,
}

impl Default for Style {
    fn default() -> Self {
        // TODO: That's temporary. Or is it?
        let style_json = include_str!("../assets/protomaps-dark.json");
        //let style_json = include_str!("../assets/protomaps-light.json");
        serde_json::from_str(style_json).expect("Failed to parse default style JSON")
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

#[derive(Deserialize, Debug)]
pub struct Color(pub Value);

impl Color {
    pub fn evaluate(&self, properties: &HashMap<String, MvtValue>, zoom: u8) -> Color32 {
        match evaluate(&self.0, properties, zoom) {
            Ok(Value::String(color)) => {
                let color: color::AlphaColor<color::Srgb> = color.parse().unwrap();
                let Rgba8 { r, g, b, a } = color.to_rgba8();
                Color32::from_rgba_premultiplied(r, g, b, a)
            }
            _ => {
                warn!(
                    "Only string color definitions are supported. Got: {:?}",
                    self.0
                );
                Color32::MAGENTA
            }
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
        let style = std::fs::read_to_string(
            env!("CARGO_MANIFEST_DIR").to_owned() + "/assets/protomaps-dark-style.json",
        )
        .unwrap();

        let _parsed_style: Style = serde_json::from_str(&style).unwrap();
    }
}
