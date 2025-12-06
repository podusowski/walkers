use std::collections::HashMap;

use color::Rgba8;
use egui::Color32;
use log::warn;
use mvt_reader::feature::Value as MvtValue;
use serde::Deserialize;
use serde_json::Value;

/// Style for rendering vector maps. Loosely (very) based on MapLibre's style specification.
#[derive(Deserialize)]
pub struct Style {
    pub layers: Vec<Layer>,
}

impl Default for Style {
    fn default() -> Self {
        // TODO: That's temporary. Or is it?
        //let style_json = include_str!("../assets/protomaps-dark-style.json");
        let style_json = include_str!("../assets/openfreemap-liberty.json");
        serde_json::from_str(style_json).expect("Failed to parse default style JSON")
    }
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Layer {
    Background,
    #[serde(rename_all = "kebab-case")]
    Fill {
        id: String,
        source_layer: String,
        filter: Option<Filter>,
        paint: Paint,
    },
    Line,
    Symbol,
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
pub struct Color(Value);

impl Color {
    pub fn evaluate(&self, properties: &HashMap<String, MvtValue>) -> Color32 {
        let value = evaluate(&self.0, properties, false).unwrap();

        let Value::String(color) = &value else {
            warn!(
                "Only string color definitions are supported. Got: {:?}",
                self.0
            );
            return Color32::MAGENTA;
        };

        let color: color::AlphaColor<color::Srgb> = color.parse().unwrap();
        let Rgba8 { r, g, b, a } = color.to_rgba8();
        Color32::from_rgba_premultiplied(r, g, b, a)
    }
}

#[derive(Deserialize, Debug)]
pub struct Opacity(Value);

impl Opacity {
    pub fn evaluate(&self, properties: &HashMap<String, MvtValue>) -> f32 {
        let value = evaluate(&self.0, properties, false).unwrap();

        match value {
            Value::Number(num) => num.as_f64().unwrap() as f32,
            other => {
                warn!("Opacity did not evaluate to a number: {:?}", other);
                1.0
            }
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct Filter(Value);

impl Filter {
    /// Match this filter against feature properties.
    pub fn matches(&self, properties: &HashMap<String, MvtValue>) -> bool {
        match evaluate(&self.0, properties, true) {
            Ok(Value::Bool(b)) => b,
            other => {
                warn!("Filter did not evaluate to a boolean: {:?}", other);
                false
            }
        }
    }
}

/// Splits a slice into its first and second element. Returns `None` if the slice does not have
/// exactly two elements.
fn split_two_element_slice<T>(slice: &[T]) -> Option<(&T, &T)> {
    if slice.len() == 2 {
        Some((&slice[0], &slice[1]))
    } else {
        None
    }
}

fn mvt_value_to_json(value: &MvtValue) -> Value {
    match value {
        MvtValue::String(s) => Value::String(s.clone()),
        MvtValue::Int(i) => Value::Number((*i).into()),
        MvtValue::Bool(b) => Value::Bool(*b),
        MvtValue::Null => Value::Null,
        _ => {
            warn!("Unsupported MVT value type: {:?}", value);
            Value::Null
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Other(String),
    #[error("Invalid expression: {0:?}")]
    InvalidExpression(Vec<Value>),
    #[error("Expected a property name or an expression, got: {0:?}")]
    ExpectedKeyOrExpression(Value),
}

/// Evaluate a style expression.
/// https://maplibre.org/maplibre-style-spec/expressions/
fn evaluate(
    value: &Value,
    properties: &HashMap<String, MvtValue>,
    filter: bool,
) -> Result<Value, Error> {
    match value {
        Value::Array(values) => {
            let (operator, arguments) = values.split_first().unwrap();
            let Value::String(operator) = operator else {
                panic!("Operator must be a string.");
            };

            match operator.as_str() {
                "literal" => {
                    if arguments.len() != 1 {
                        panic!("'literal' operator requires exactly one argument.");
                    }

                    if !arguments[0].is_array() {
                        panic!("'literal' operator argument must be an array.");
                    }

                    Ok(arguments[0].clone())
                }
                "get" => {
                    if arguments.len() != 1 {
                        panic!("'get' operator requires exactly one argument.");
                    }

                    let Value::String(key) = &arguments[0] else {
                        panic!("'get' operator argument must be a string.");
                    };

                    match properties.get(key) {
                        Some(MvtValue::String(s)) => Ok(Value::String(s.clone())),
                        Some(MvtValue::Int(i)) => Ok(Value::Number((*i).into())),
                        None => Ok(Value::Null),
                        _ => {
                            panic!("Unsupported property value type for 'get' operator.");
                        }
                    }
                }
                "match" => {
                    let (value, arms) = arguments.split_first().unwrap();
                    let evaluated_value = evaluate(value, properties, filter)?;
                    for arm in arms.chunks(2) {
                        let arm_value = &arm[0];
                        let arm_result = &arm[1];

                        if evaluated_value == *arm_value {
                            return evaluate(arm_result, properties, filter);
                        }
                    }
                    todo!("No match found in 'match' expression.");
                }
                "case" => {
                    for arm in arguments.chunks(2) {
                        match arm.iter().as_slice() {
                            [condition, value] => {
                                let evaluated_condition = evaluate(condition, properties, filter)?;
                                if let Value::Bool(true) = evaluated_condition {
                                    return evaluate(value, properties, filter);
                                }
                            }
                            [default] => {
                                return evaluate(default, properties, filter);
                            }
                            _ => {
                                panic!("Invalid 'case' arm.");
                            }
                        }
                    }
                    todo!("No true condition found in 'case' expression.");
                }
                "in" => {
                    let (value, list) = arguments.split_first().unwrap();

                    let evaluated_value = if filter {
                        mvt_value_to_json(properties.get(value.as_str().unwrap()).unwrap())
                    } else {
                        evaluate(value, properties, filter)?
                    };

                    for item in list {
                        if evaluated_value == evaluate(item, properties, filter)? {
                            return Ok(Value::Bool(true));
                        }
                    }
                    Ok(Value::Bool(false))
                }
                "==" => {
                    let (left, right) = split_two_element_slice(arguments).unwrap();
                    let left = property_or_expression(left, properties, filter)?;
                    Ok(Value::Bool(left == *right))
                }
                "!=" => {
                    let (left, right) = split_two_element_slice(arguments).unwrap();
                    let left = property_or_expression(left, properties, filter)?;
                    Ok(Value::Bool(left != *right))
                }
                "any" => Ok(arguments
                    .iter()
                    .any(|value| evaluate(value, properties, filter).unwrap() == Value::Bool(true))
                    .into()),
                operator => {
                    warn!("Unsupported operator: {}", operator);
                    Ok(Value::Null)
                }
                "interpolate" => {
                    let (interpolation_type, args) = arguments.split_first().unwrap();
                    let (input, stops) = args.split_first().unwrap();
                    let evaluated_input = evaluate(input, properties, filter)?;

                    Ok(Value::Null) // TODO: Implement interpolate
                }
            }
        }
        primitive => Ok(primitive.clone()),
    }
}

fn property_or_expression(
    value: &Value,
    properties: &HashMap<String, MvtValue>,
    filter: bool,
) -> Result<Value, Error> {
    match value {
        Value::String(key) => {
            Ok(mvt_value_to_json(properties.get(key).ok_or(
                Error::Other(format!("Property '{key}' not found")),
            )?))
        }
        Value::Array(_) => evaluate(&value, properties, filter),
        _ => Err(Error::ExpectedKeyOrExpression(value.clone())),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use super::*;

    #[test]
    fn test_style_parsing() {
        let style = std::fs::read_to_string(
            env!("CARGO_MANIFEST_DIR").to_owned() + "/assets/protomaps-dark-style.json",
        )
        .unwrap();

        let _parsed_style: Style = serde_json::from_str(&style).unwrap();
    }

    #[test]
    fn test_eq_filter_matching() {
        let park = HashMap::from([("type".to_string(), MvtValue::String("park".to_string()))]);
        let forest = HashMap::from([("type".to_string(), MvtValue::String("forest".to_string()))]);

        let filter = Filter(json!(["==", "type", "park"]));

        assert!(filter.matches(&park));
        assert!(!filter.matches(&forest));
    }

    #[test]
    fn test_in_filter() {
        let park = HashMap::from([("type".to_string(), MvtValue::String("park".to_string()))]);
        let road = HashMap::from([("type".to_string(), MvtValue::String("road".to_string()))]);

        let filter = Filter(json!(["in", "type", "park", "forest"]));

        assert!(filter.matches(&park));
        assert!(!filter.matches(&road));
    }

    #[test]
    fn test_evaluate_color() {
        assert_eq!(
            Color(Value::String("#ffffff".to_string())).evaluate(&HashMap::new()),
            Color32::WHITE
        );

        assert_eq!(
            Color(Value::String("red".to_string())).evaluate(&HashMap::new()),
            Color32::RED
        );
    }

    #[test]
    fn test_literal_operator() {
        let properties = HashMap::new();

        assert_eq!(
            evaluate(&json!(["literal", [1, 2, 3]]), &properties, false).unwrap(),
            json!([1, 2, 3])
        );
    }

    #[test]
    fn test_get_operator() {
        let properties =
            HashMap::from([("name".to_string(), MvtValue::String("Polska".to_string()))]);

        assert_eq!(
            evaluate(&json!(["get", "name"]), &properties, false).unwrap(),
            json!("Polska")
        );
    }

    #[test]
    fn test_match_operator() {
        let properties = HashMap::new();

        assert_eq!(
            evaluate(
                &json!([
                    "match",
                    42,
                    1,
                    "Not this one",
                    2,
                    "Also not this one",
                    42,
                    "Got it!",
                ]),
                &properties,
                false
            )
            .unwrap(),
            json!("Got it!")
        );
    }

    #[test]
    fn test_case_operator() {
        let properties = HashMap::new();

        assert_eq!(
            evaluate(
                &json!([
                    "case",
                    false,
                    "Not this one",
                    false,
                    "Also not this one",
                    true,
                    "Got it!",
                ]),
                &properties,
                false
            )
            .unwrap(),
            json!("Got it!")
        );

        assert_eq!(
            evaluate(
                &json!(["case", false, "first", false, "second", "default"]),
                &properties,
                false
            )
            .unwrap(),
            json!("default")
        );
    }

    #[test]
    fn test_in_operator() {
        let properties = HashMap::new();

        assert_eq!(
            evaluate(&json!(["in", 1, 1, 2, 3,]), &properties, false).unwrap(),
            json!(true)
        );

        assert_eq!(
            evaluate(&json!(["in", 4, 1, 2, 3,]), &properties, false).unwrap(),
            json!(false)
        );
    }

    #[test]
    fn test_any_operator() {
        let properties = HashMap::new();

        assert_eq!(
            evaluate(&json!(["any", true, false]), &properties, false).unwrap(),
            json!(true)
        );

        assert_eq!(
            evaluate(&json!(["any", false, false]), &properties, false).unwrap(),
            json!(false)
        );
    }

    #[test]
    fn test_interpolate_operator() {
        // https://maplibre.org/maplibre-style-spec/expressions/#interpolate
        let properties = HashMap::from([("zoom".to_string(), MvtValue::Int(5))]);

        assert_eq!(
            evaluate(
                &json!(["interpolate", ["linear"], ["get", "zoom"], 0, 1, 10, 2]),
                &properties,
                false
            )
            .unwrap(),
            json!(1.5)
        );
    }
}
