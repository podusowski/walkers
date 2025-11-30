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
        let style_json = include_str!("../assets/protomaps-dark-style.json");
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
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct Paint {
    pub fill_color: Option<Color>,
}

#[derive(Deserialize, Debug)]
pub struct Color(Value);

impl Color {
    pub fn evaluate(&self, properties: &HashMap<String, MvtValue>) -> Color32 {
        let value = evaluate(&self.0, properties);

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
pub struct Filter(Vec<Value>);

impl Filter {
    /// Match this filter against feature properties.
    pub fn matches(&self, properties: &HashMap<String, MvtValue>) -> bool {
        let (function, args) = self.0.split_first().unwrap();
        match function {
            Value::String(op) if op == "==" => {
                let (key, arg) = split_two_element_slice(args).unwrap();
                let Value::String(key) = key else { todo!() };

                properties.get(key) == Some(&MvtValue::String(arg.as_str().unwrap().to_string()))
            }
            Value::String(op) if op == "in" => {
                let (key, values) = args.split_first().unwrap();
                let Value::String(key) = key else { todo!() };

                let properties_value = properties.get(key).unwrap();
                values
                    .iter()
                    .any(|filter_value| eq(filter_value, properties_value))
            }
            f => {
                log::warn!("Unsupported filter function: {f:?}");
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

fn eq(a: &Value, b: &MvtValue) -> bool {
    match (a, b) {
        (Value::String(a), MvtValue::String(b)) => a == b,
        (Value::Number(a), MvtValue::Int(b)) => a.as_i64() == Some(*b),
        _ => false,
    }
}

/// Evaluate a style expression.
/// https://maplibre.org/maplibre-style-spec/expressions/
fn evaluate(value: &Value, properties: &HashMap<String, MvtValue>) -> Value {
    match value {
        Value::Array(values) => {
            let (operator, arguments) = values.split_first().unwrap();
            let Value::String(operator) = operator else {
                panic!("Operator must be a string.");
            };

            match operator.as_str() {
                "get" => {
                    if arguments.len() != 1 {
                        panic!("'get' operator requires exactly one argument.");
                    }

                    let Value::String(key) = &arguments[0] else {
                        panic!("'get' operator argument must be a string.");
                    };

                    match properties.get(key) {
                        Some(MvtValue::String(s)) => Value::String(s.clone()),
                        Some(MvtValue::Int(i)) => Value::Number((*i).into()),
                        None => Value::Null,
                        _ => {
                            panic!("Unsupported property value type for 'get' operator.");
                        }
                    }
                }
                "match" => {
                    let (value, arms) = arguments.split_first().unwrap();
                    let evaluated_value = evaluate(value, properties);
                    for arm in arms.chunks(2) {
                        let arm_value = &arm[0];
                        let arm_result = &arm[1];

                        if evaluated_value == *arm_value {
                            return evaluate(arm_result, properties);
                        }
                    }
                    todo!("No match found in 'match' expression.");
                }
                "case" => {
                    for arm in arguments.chunks(2) {
                        match arm.iter().as_slice() {
                            [condition, value] => {
                                let evaluated_condition = evaluate(condition, properties);
                                if let Value::Bool(true) = evaluated_condition {
                                    return evaluate(value, properties);
                                }
                            }
                            [default] => {
                                return evaluate(default, properties);
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
                    let evaluated_value = evaluate(value, properties);
                    for item in list {
                        if evaluated_value == evaluate(item, properties) {
                            return Value::Bool(true);
                        }
                    }
                    Value::Bool(false)
                }
                operator => {
                    warn!("Unsupported operator: {}", operator);
                    Value::Null
                }
            }
        }
        primitive => primitive.clone(),
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

        let filter = Filter(vec![
            Value::String("==".to_string()),
            Value::String("type".to_string()),
            Value::String("park".to_string()),
        ]);

        assert!(filter.matches(&park));
        assert!(!filter.matches(&forest));
    }

    #[test]
    fn test_in_filter() {
        let park = HashMap::from([("type".to_string(), MvtValue::String("park".to_string()))]);
        let road = HashMap::from([("type".to_string(), MvtValue::String("road".to_string()))]);

        let filter = Filter(vec![
            Value::String("in".to_string()),
            Value::String("type".to_string()),
            Value::String("park".to_string()),
            Value::String("forest".to_string()),
        ]);

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
    fn test_get_operator() {
        let properties =
            HashMap::from([("name".to_string(), MvtValue::String("Polska".to_string()))]);

        assert_eq!(
            evaluate(&json!(["get", "name"]), &properties),
            Value::String("Polska".to_string())
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
                &properties
            ),
            Value::String("Got it!".to_string())
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
                &properties
            ),
            Value::String("Got it!".to_string())
        );

        assert_eq!(
            evaluate(
                &json!(["case", false, "first", false, "second", "default"]),
                &properties
            ),
            json!("default")
        );
    }

    #[test]
    fn test_in_operator() {
        let properties = HashMap::new();

        assert_eq!(
            evaluate(&json!(["in", 1, 1, 2, 3,]), &properties),
            json!(true)
        );

        assert_eq!(
            evaluate(&json!(["in", 4, 1, 2, 3,]), &properties),
            json!(false)
        );
    }
}
