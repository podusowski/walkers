use std::collections::HashMap;

use mvt_reader::feature::Value as MvtValue;
use serde_json::Value;

/// Style for rendering vector maps. Loosely (very) based on MapLibre's style specification.
#[derive(serde::Deserialize)]
pub struct Style {
    pub layers: Vec<Layer>,
}

impl Default for Style {
    fn default() -> Self {
        // TODO: That's temporary. Or is it?
        let style_json = include_str!("../assets/protonmaps-dark-style.json");
        serde_json::from_str(style_json).expect("Failed to parse default style JSON")
    }
}

#[derive(serde::Deserialize, Debug)]
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

#[derive(serde::Deserialize, Debug)]
pub struct Paint {
    pub fill_color: Option<Vec<Value>>,
}

#[derive(serde::Deserialize, Debug)]
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_style_parsing() {
        let style = std::fs::read_to_string(
            env!("CARGO_MANIFEST_DIR").to_owned() + "/assets/protonmaps-dark-style.json",
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
}
