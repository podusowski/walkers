//! Evaluate MapLibre style expressions.
//! <https://maplibre.org/maplibre-style-spec/expressions/>

use color::{AlphaColor, HueDirection, Srgb};
use log::warn;
use mvt_reader::feature::Value as MvtValue;
use serde_json::{Number, Value};
use std::collections::HashMap;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid expression: {0:?}")]
    InvalidExpression(Value),
    #[error("Expected a property name or an expression, got: {0:?}")]
    ExpectedKeyOrExpression(Value),
    #[error("Interpolate stop not found for input value: {0}. Expression: {1}")]
    InterpolateStopNotFound(Value, Value),
    #[error("Cannot interpolate between values: {0} and {1}")]
    CannotInterpolate(Value, Value),
    #[error("Single string expected, got: {0:?}")]
    SingleStringExpected(Vec<Value>),
    #[error("Single array expected, got: {0:?}")]
    SingleArrayExpected(Vec<Value>),
    #[error("Single value expected, got: {0:?}")]
    SingleValueExpected(Vec<Value>),
    #[error("Exactly two elemented expected, got: {0:?}")]
    TwoElementsExpected(Vec<Value>),
    #[error("At least two elemented expected, got: {0:?}")]
    AtLeastTwoElementsExpected(Vec<Value>),
    #[error("Property '{0}' missing in {1:?}")]
    PropertyMissing(String, HashMap<String, MvtValue>),
    #[error("Value must be a number, got: {0}")]
    ExpectedNumber(Value),
    #[error("Number must be a float, got: {0}")]
    ExpectedFloat(Number),
    #[error("Could not serialize a float. Is it NaN?")]
    CouldNotSerializeFloat,
    #[error(transparent)]
    ColorParse(color::ParseError),
}

/// Evaluate a style expression.
/// https://maplibre.org/maplibre-style-spec/expressions/
pub fn evaluate(
    value: &Value,
    properties: &HashMap<String, MvtValue>,
    zoom: u8,
) -> Result<Value, Error> {
    match value {
        Value::Array(values) => {
            let Some((Value::String(operator), arguments)) = values.split_first() else {
                return Err(Error::InvalidExpression(value.clone()));
            };

            match operator.as_str() {
                "zoom" => Ok(Value::Number((zoom as i64).into())),
                "literal" => single_array(arguments),
                "!" => match evaluate(&single_value(arguments)?, properties, zoom)? {
                    Value::Bool(b) => Ok(Value::Bool(!b)),
                    _ => Err(Error::InvalidExpression(value.clone())),
                },
                "get" => {
                    let key = single_string(arguments)?;
                    Ok(properties.get(key).map_or(Value::Null, mvt_value_to_json))
                }
                "has" => Ok(Value::Bool(
                    properties.contains_key(single_string(arguments)?),
                )),
                "!has" => Ok(Value::Bool(
                    !properties.contains_key(single_string(arguments)?),
                )),
                "match" => {
                    let (value, arms) = first_and_rest(arguments)?;
                    let evaluated_value = evaluate(value, properties, zoom)?;
                    for arm in arms.chunks(2) {
                        if arm.len() == 1 {
                            // Default case
                            return evaluate(&arm[0], properties, zoom);
                        }

                        let arm_value = &arm[0];
                        let arm_result = &arm[1];

                        if evaluated_value == *arm_value {
                            return evaluate(arm_result, properties, zoom);
                        }
                    }
                    todo!("No match found in 'match' expression.");
                }
                "case" => {
                    for arm in arguments.chunks(2) {
                        match arm.iter().as_slice() {
                            [condition, value] => {
                                let evaluated_condition = evaluate(condition, properties, zoom)?;
                                if let Value::Bool(true) = evaluated_condition {
                                    return evaluate(value, properties, zoom);
                                }
                            }
                            [default] => {
                                return evaluate(default, properties, zoom);
                            }
                            _ => {
                                panic!("Invalid 'case' arm.");
                            }
                        }
                    }
                    todo!("No true condition found in 'case' expression.");
                }
                "coalesce" => {
                    for argument in arguments {
                        match evaluate(argument, properties, zoom)? {
                            Value::Null => continue,
                            non_null => return Ok(non_null),
                        }
                    }
                    Ok(Value::Null)
                }
                "in" => {
                    let (value, list) = first_and_rest(arguments)?;
                    let value = property_or_expression(value, properties, zoom)?;

                    for item in list {
                        if value == evaluate(item, properties, zoom)? {
                            return Ok(Value::Bool(true));
                        }
                    }

                    Ok(Value::Bool(false))
                }
                "==" => {
                    let (left, right) = two_elements(arguments)?;
                    let left = property_or_expression(left, properties, zoom)?;
                    Ok(Value::Bool(left == *right))
                }
                "!=" => {
                    let (left, right) = two_elements(arguments)?;
                    let left = property_or_expression(left, properties, zoom)?;
                    Ok(Value::Bool(left != *right))
                }
                "any" => Ok(arguments
                    .iter()
                    .try_fold(false, |acc, value| {
                        Ok(acc || evaluate(value, properties, zoom)? == Value::Bool(true))
                    })?
                    .into()),
                "all" => Ok(arguments
                    .iter()
                    .try_fold(true, |acc, value| {
                        Ok(acc && evaluate(value, properties, zoom)? == Value::Bool(true))
                    })?
                    .into()),
                "interpolate" => {
                    let (_interpolation_type, args) = first_and_rest(arguments)?;
                    let (input, stops) = first_and_rest(args)?;
                    let input = evaluate(input, properties, zoom)?;

                    // Stops are pairs of [input, output].
                    let stops = stops
                        .chunks(2)
                        .map(|chunk| (chunk[0].clone(), chunk[1].clone()))
                        .collect::<Vec<_>>();

                    // Find the two stops surrounding the input value.
                    let stop_pair = stops.windows(2).find(|pair| {
                        let left_stop = &pair[0].0;
                        let right_stop = &pair[1].0;
                        lte(left_stop, &input) && lte(&input, right_stop)
                    });

                    if let Some(stop_pair) = stop_pair {
                        let input_delta = numeric_difference(&stop_pair[1].0, &stop_pair[0].0)?;

                        // Position of the input value between the two stops (0.0 to 1.0).
                        let input_position =
                            numeric_difference(&input, &stop_pair[0].0)? / input_delta;

                        let result = lerp(&stop_pair[0].1, &stop_pair[1].1, input_position)?;
                        Ok(result)
                    } else if lt(&input, &stops[0].0) {
                        Ok(stops[0].1.clone())
                    } else if lt(&stops[stops.len() - 1].0, &input) {
                        Ok(stops[stops.len() - 1].1.clone())
                    } else {
                        Err(Error::InterpolateStopNotFound(input, value.clone()))
                    }
                }
                "format" => {
                    let mut result = String::new();
                    for argument in arguments.chunks(2) {
                        let (input, _style_override) = two_elements(argument)?;
                        result.push_str(
                            evaluate(input, properties, zoom)?
                                .as_str()
                                .ok_or(Error::InvalidExpression(value.clone()))?,
                        );
                    }
                    Ok(Value::String(result))
                }
                _ => Err(Error::InvalidExpression(value.clone())),
            }
        }
        primitive => Ok(primitive.clone()),
    }
}

fn mvt_value_to_json(value: &MvtValue) -> Value {
    match value {
        MvtValue::String(s) => Value::String(s.clone()),
        MvtValue::Int(i) => Value::Number((*i).into()),
        MvtValue::Bool(b) => Value::Bool(*b),
        MvtValue::Null => Value::Null,
        _ => {
            warn!("Unsupported MVT value type: {value:?}");
            Value::Null
        }
    }
}

/// Expect a float Value.
fn float(v: &Value) -> Result<f64, Error> {
    if let Value::Number(n) = v {
        n.as_f64().ok_or(Error::ExpectedFloat(n.clone()))
    } else {
        Err(Error::ExpectedNumber(v.clone()))
    }
}

/// Linear interpolation between two Values (Numbers or Strings representing colors).
fn lerp(a: &Value, b: &Value, t: f64) -> Result<Value, Error> {
    match (a, b) {
        (Value::String(a), Value::String(b)) => {
            let a: AlphaColor<Srgb> = a.parse().map_err(Error::ColorParse)?;
            let b: AlphaColor<Srgb> = b.parse().map_err(Error::ColorParse)?;
            let color = a.lerp(b, t as f32, HueDirection::default());
            Ok(Value::String(color.to_rgba8().to_string()))
        }
        (Value::Number(a), Value::Number(b)) => {
            let a = a.as_f64().ok_or(Error::ExpectedFloat(a.clone()))?;
            let b = b.as_f64().ok_or(Error::ExpectedFloat(b.clone()))?;
            Ok(Value::Number(
                Number::from_f64(a + (b - a) * t).ok_or(Error::CouldNotSerializeFloat)?,
            ))
        }
        _ => Err(Error::CannotInterpolate(a.clone(), b.clone())),
    }
}

fn numeric_difference(left: &Value, right: &Value) -> Result<f64, Error> {
    Ok(float(left)? - float(right)?)
}

/// Less than comparison for Numbers and Strings.
fn lt(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(l), Value::Number(r)) => l.as_i64() < r.as_i64(),
        (Value::String(l), Value::String(r)) => l < r,
        _ => false,
    }
}

/// Less than or equal comparison for Numbers and Strings.
fn lte(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(l), Value::Number(r)) => l.as_i64() <= r.as_i64(),
        (Value::String(l), Value::String(r)) => l <= r,
        _ => false,
    }
}

/// Evaluate token as either a property key (String) or an expression (Array).
fn property_or_expression(
    value: &Value,
    properties: &HashMap<String, MvtValue>,
    zoom: u8,
) -> Result<Value, Error> {
    match value {
        Value::String(key) => {
            Ok(mvt_value_to_json(properties.get(key).ok_or(
                Error::PropertyMissing(key.clone(), properties.clone()),
            )?))
        }
        Value::Array(_) => evaluate(value, properties, zoom),
        _ => Err(Error::ExpectedKeyOrExpression(value.clone())),
    }
}

/// Expect exactly one string element.
fn single_string(values: &[Value]) -> Result<&str, Error> {
    if let [Value::String(s)] = values {
        Ok(s)
    } else {
        Err(Error::SingleStringExpected(values.to_vec()))
    }
}

/// Expect exactly one array element.
fn single_array(values: &[Value]) -> Result<Value, Error> {
    match values {
        [arr] if arr.is_array() => Ok(arr.clone()),
        _ => Err(Error::SingleArrayExpected(values.to_vec())),
    }
}

/// Expect exactly one element.
fn single_value(values: &[Value]) -> Result<Value, Error> {
    match values {
        [value] => Ok(value.clone()),
        _ => Err(Error::SingleValueExpected(values.to_vec())),
    }
}

/// Expect exactly two elements.
fn two_elements(slice: &[Value]) -> Result<(&Value, &Value), Error> {
    if let [a, b] = slice {
        Ok((a, b))
    } else {
        Err(Error::TwoElementsExpected(slice.to_vec()))
    }
}

/// Expect two or more elements.
fn first_and_rest(slice: &[Value]) -> Result<(&Value, &[Value]), Error> {
    slice
        .split_first()
        .ok_or(Error::AtLeastTwoElementsExpected(slice.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{Color, Filter};
    use egui::Color32;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn test_lerp() {
        assert_eq!(5.0, lerp(&json!(0), &json!(10.0), 0.5).unwrap());

        assert_eq!(
            "rgb(128, 128, 128)",
            lerp(&json!("rgb(0, 0, 0)"), &json!("rgb(255, 255, 255)"), 0.5).unwrap()
        );
    }

    #[test]
    fn test_eq_filter_matching() {
        let park = HashMap::from([("type".to_string(), MvtValue::String("park".to_string()))]);
        let forest = HashMap::from([("type".to_string(), MvtValue::String("forest".to_string()))]);

        let filter = Filter(json!(["==", "type", "park"]));

        assert!(filter.matches(&park, 1));
        assert!(!filter.matches(&forest, 1));
    }

    #[test]
    fn test_in_filter() {
        let park = HashMap::from([("type".to_string(), MvtValue::String("park".to_string()))]);
        let road = HashMap::from([("type".to_string(), MvtValue::String("road".to_string()))]);

        let filter = Filter(json!(["in", "type", "park", "forest"]));

        assert!(filter.matches(&park, 1));
        assert!(!filter.matches(&road, 1));
    }

    #[test]
    fn test_evaluate_color() {
        assert_eq!(
            Color(Value::String("#ffffff".to_string())).evaluate(&HashMap::new(), 1),
            Color32::WHITE
        );

        assert_eq!(
            Color(Value::String("red".to_string())).evaluate(&HashMap::new(), 1),
            Color32::RED
        );
    }

    #[test]
    fn test_literal_operator() {
        let properties = HashMap::new();

        assert_eq!(
            evaluate(&json!(["literal", [1, 2, 3]]), &properties, 1).unwrap(),
            json!([1, 2, 3])
        );
    }

    #[test]
    fn test_get_operator() {
        let properties =
            HashMap::from([("name".to_string(), MvtValue::String("Polska".to_string()))]);

        assert_eq!(
            evaluate(&json!(["get", "name"]), &properties, 1).unwrap(),
            json!("Polska")
        );

        assert_eq!(
            evaluate(&json!(["get", "population"]), &properties, 1).unwrap(),
            Value::Null
        );
    }

    #[test]
    fn test_has_operator() {
        let properties =
            HashMap::from([("name".to_string(), MvtValue::String("Polska".to_string()))]);

        assert_eq!(
            evaluate(&json!(["has", "name"]), &properties, 1,).unwrap(),
            json!(true)
        );
    }

    #[test]
    fn test_not_has_operator() {
        assert_eq!(
            evaluate(&json!(["!has", "name"]), &HashMap::new(), 1,).unwrap(),
            json!(true)
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
                1,
            )
            .unwrap(),
            json!("Got it!")
        );
    }

    #[test]
    fn test_match_operator_reaching_default() {
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
                    "It's the default!",
                ]),
                &properties,
                1,
            )
            .unwrap(),
            json!("It's the default!")
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
                1,
            )
            .unwrap(),
            json!("Got it!")
        );

        assert_eq!(
            evaluate(
                &json!(["case", false, "first", false, "second", "default"]),
                &properties,
                1,
            )
            .unwrap(),
            json!("default")
        );
    }

    #[test]
    fn test_coalesce_operator() {
        let properties = HashMap::new();

        assert_eq!(
            evaluate(&json!(["coalesce", Value::Null, "Got it!"]), &properties, 1,).unwrap(),
            json!("Got it!")
        );

        assert_eq!(
            evaluate(
                &json!(["coalesce", Value::Null, Value::Null]),
                &properties,
                1,
            )
            .unwrap(),
            Value::Null
        );
    }

    #[test]
    fn test_in_operator() {
        let properties =
            HashMap::from([("name".to_string(), MvtValue::String("Polska".to_string()))]);

        assert_eq!(
            evaluate(
                &json!(["in", "name", "one", "two", "Polska", "three"]),
                &properties,
                1,
            )
            .unwrap(),
            json!(true)
        );

        assert_eq!(
            evaluate(
                &json!(["in", "name", "one", "two", "three"]),
                &properties,
                1,
            )
            .unwrap(),
            json!(false)
        );
    }

    #[test]
    fn test_any_operator() {
        let properties = HashMap::new();

        assert_eq!(
            evaluate(&json!(["any", true, false]), &properties, 1,).unwrap(),
            json!(true)
        );

        assert_eq!(
            evaluate(&json!(["any", false, false]), &properties, 1,).unwrap(),
            json!(false)
        );
    }

    #[test]
    fn test_all_operator() {
        let properties = HashMap::new();

        assert_eq!(
            evaluate(&json!(["all", true, false]), &properties, 1,).unwrap(),
            json!(false)
        );

        assert_eq!(
            evaluate(&json!(["all", true, true]), &properties, 1,).unwrap(),
            json!(true)
        );
    }

    #[test]
    fn test_interpolate_operator() {
        // https://maplibre.org/maplibre-style-spec/expressions/#interpolate
        let properties = HashMap::from([("zoom".to_string(), MvtValue::Int(5))]);

        assert_eq!(
            evaluate(
                &json!(["interpolate", ["linear"], 5, 0, 0, 10, 10]),
                &properties,
                1,
            )
            .unwrap(),
            json!(5.0)
        );
    }

    #[test]
    fn test_negation_operator() {
        let properties = HashMap::new();

        assert_eq!(
            evaluate(&json!(["!", false]), &properties, 1,).unwrap(),
            json!(true)
        );
    }

    #[test]
    fn test_format_operator() {
        let properties = HashMap::new();

        assert_eq!(
            evaluate(&json!(["format", "Hello", {}, "World", {}]), &properties, 1,).unwrap(),
            json!("HelloWorld")
        );
    }
}
