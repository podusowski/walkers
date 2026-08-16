pub mod basemap;

use color::Rgba8;
use egui::Color32;
use log::warn;
use serde::Deserialize;
pub use serde_json::{Value, json};
use thiserror::Error;

use crate::expression::Context;

/// Style for rendering vector maps.
///
/// It is based on MapLibre's style specification, but only a small subset is supported.
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
        let style_json = include_str!("../../assets/protomaps-dark.json");
        serde_json::from_str(style_json).expect("failed to parse style JSON")
    }

    /// Style based on Protomaps Dark Vis flavour. Requires Protomaps source.
    ///
    /// <https://docs.protomaps.com/basemaps/flavors>
    pub fn protomaps_dark_vis() -> Self {
        let style_json = include_str!("../../assets/protomaps-dark-vis.json");
        serde_json::from_str(style_json).expect("failed to parse style JSON")
    }

    /// Style based on Protomaps Light flavour. Requires Protomaps source.
    ///
    /// <https://docs.protomaps.com/basemaps/flavors>
    pub fn protomaps_light() -> Self {
        let style_json = include_str!("../../assets/protomaps-light.json");
        serde_json::from_str(style_json).expect("failed to parse style JSON")
    }

    pub fn openfreemap_bright() -> Self {
        let style_json = include_str!("../../assets/openfreemap-bright.json");
        serde_json::from_str(style_json).expect("failed to parse style JSON")
    }
}

/// Which layer, or layers, of a vector tile a style layer draws from.
///
/// MapLibre names exactly one. Walkers also takes a list, for schemas which spread one concept
/// over several layers, and treats an empty name or an empty list as "all of them".
#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum SourceLayer {
    One(String),
    Many(Vec<String>),
}

impl SourceLayer {
    /// Whether a tile layer of this name should be drawn.
    pub fn matches(&self, name: &str) -> bool {
        match self {
            SourceLayer::One(one) => one.is_empty() || one == name,
            SourceLayer::Many(many) => many.is_empty() || many.iter().any(|it| it == name),
        }
    }

    /// Matching everything is intended for sparse overlay tiles. Pointing dense basemap rules
    /// at it would scan every layer of every tile.
    pub fn is_all(&self) -> bool {
        match self {
            SourceLayer::One(one) => one.is_empty(),
            SourceLayer::Many(many) => many.is_empty(),
        }
    }
}

impl std::fmt::Display for SourceLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceLayer::One(one) => write!(f, "'{one}'"),
            SourceLayer::Many(many) => write!(f, "{many:?}"),
        }
    }
}

impl From<&str> for SourceLayer {
    fn from(name: &str) -> Self {
        SourceLayer::One(name.to_owned())
    }
}

impl From<String> for SourceLayer {
    fn from(name: String) -> Self {
        SourceLayer::One(name)
    }
}

impl<const N: usize> From<[&str; N]> for SourceLayer {
    fn from(names: [&str; N]) -> Self {
        SourceLayer::Many(names.iter().map(|name| (*name).to_owned()).collect())
    }
}

impl From<&[&str]> for SourceLayer {
    fn from(names: &[&str]) -> Self {
        SourceLayer::Many(names.iter().map(|name| (*name).to_owned()).collect())
    }
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Layer {
    Background {
        paint: Paint,
    },
    #[serde(rename_all = "kebab-case")]
    Fill {
        source_layer: SourceLayer,
        filter: Option<Filter>,
        paint: Paint,
    },
    #[serde(rename_all = "kebab-case")]
    Line {
        source_layer: SourceLayer,
        filter: Option<Filter>,
        paint: Paint,
    },
    #[serde(rename_all = "kebab-case")]
    Symbol {
        source_layer: SourceLayer,
        filter: Option<Filter>,
        layout: Layout,
        paint: Option<Paint>,
    },
    Circle {
        source_layer: SourceLayer,
        filter: Option<Filter>,
    },
    Raster,
    FillExtrusion,
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct Paint {
    pub background_color: Option<Color>,
    pub fill_color: Option<Color>,
    /// <https://maplibre.org/maplibre-style-spec/layers/>#fill-opacity
    pub fill_opacity: Option<Float>,
    pub line_width: Option<Float>,
    /// <https://maplibre.org/maplibre-style-spec/layers/>#line-color
    pub line_color: Option<Color>,
    /// <https://maplibre.org/maplibre-style-spec/layers/>#line-opacity
    pub line_opacity: Option<Float>,
    /// <https://maplibre.org/maplibre-style-spec/layers/>#line-dasharray
    pub line_dasharray: Option<Dasharray>,
    /// <https://maplibre.org/maplibre-style-spec/layers/>#text-color
    pub text_color: Option<Color>,
    /// <https://maplibre.org/maplibre-style-spec/layers/>#text-halo-color
    pub text_halo_color: Option<Color>,
    /// <https://maplibre.org/maplibre-style-spec/layers/>#text-halo-width
    pub text_halo_width: Option<Float>,
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
            Ok(value) => value,
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

/// A dash/gap pattern for lines. Values are in units of the line's width, per the
/// MapLibre spec, e.g. `[2, 1]` draws a dash twice as long as the gap.
#[derive(Deserialize, Debug)]
pub struct Dasharray(pub Value);

impl Dasharray {
    pub fn evaluate(&self, context: &Context) -> Option<Vec<f32>> {
        match self.try_evaluate(context) {
            Ok(pattern) => Some(pattern),
            Err(err) => {
                warn!("{err}");
                None
            }
        }
    }

    fn try_evaluate(&self, context: &Context) -> Result<Vec<f32>, StyleError> {
        // A plain array of numbers, e.g. `[2, 1]`, is not a valid expression on its own
        // (expressions are arrays starting with an operator string), so it must be
        // recognized before falling back to `Context::evaluate`.
        let value = match &self.0 {
            Value::Array(values) if values.first().is_some_and(Value::is_number) => self.0.clone(),
            other => context.evaluate(other)?,
        };

        match value {
            Value::Array(values) => values
                .iter()
                .map(|v| v.as_f64().map(|f| f as f32).ok_or(StyleError::InvalidType))
                .collect(),
            _ => Err(StyleError::InvalidType),
        }
    }
}

/// Build an `["interpolate", ["linear"], ["zoom"], ...]` expression from its stops.
pub fn linear_zoom_interpolation(stops: &[(f64, f64)]) -> Float {
    let mut expression = vec![json!("interpolate"), json!(["linear"]), json!(["zoom"])];

    for &(zoom, value) in stops {
        expression.push(json!(zoom));
        expression.push(json!(value));
    }

    Float(json!(expression))
}

#[derive(Deserialize, Debug)]
pub struct Filter(pub Value);

impl Filter {
    /// Match this filter against feature properties.
    pub fn matches(&self, context: &Context) -> bool {
        match context.evaluate(&self.0) {
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
    pub text_field: Option<Value>,
    pub text_size: Option<Float>,
}

impl Layout {
    pub fn text(&self, context: &Context) -> Option<String> {
        self.text_field
            .as_ref()
            .and_then(|value| match context.evaluate(value) {
                Ok(Value::String(s)) => Some(s),
                _ => None,
            })
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

#[cfg(test)]
mod source_layer_tests {
    use super::*;

    /// A plain name is what every MapLibre style writes, and must keep working.
    #[test]
    fn one_name_matches_only_itself() {
        let one: SourceLayer = "roads".into();
        assert!(one.matches("roads"));
        assert!(!one.matches("transportation"));
        assert!(!one.is_all());
    }

    #[test]
    fn a_list_matches_any_of_its_names() {
        let many: SourceLayer = ["landcover", "landuse", "park"].into();
        assert!(many.matches("landcover"));
        assert!(many.matches("park"));
        assert!(!many.matches("roads"));
        assert!(!many.is_all());
    }

    /// Emptiness has meant "every layer of the tile" since before this type existed.
    #[test]
    fn emptiness_matches_everything() {
        for empty in [SourceLayer::from(""), SourceLayer::Many(Vec::new())] {
            assert!(empty.matches("anything"));
            assert!(empty.is_all());
        }
    }

    #[test]
    fn both_forms_deserialize() {
        let one: SourceLayer = serde_json::from_str(r#""roads""#).unwrap();
        assert!(one.matches("roads"));

        let many: SourceLayer = serde_json::from_str(r#"["roads", "transportation"]"#).unwrap();
        assert!(many.matches("transportation"));
    }
}
