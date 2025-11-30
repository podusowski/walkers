use std::collections::HashMap;

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

#[derive(serde::Deserialize)]
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

#[derive(serde::Deserialize)]
pub struct Paint {
    pub fill_color: Option<Vec<serde_json::Value>>,
}

#[derive(serde::Deserialize)]
pub struct Filter(Vec<serde_json::Value>);

impl Filter {
    pub fn matches(&self, properties: &HashMap<String, mvt_reader::feature::Value>) -> bool {
        todo!()
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
    fn test_filter_matching() {
        let park = HashMap::from([(
            "type".to_string(),
            mvt_reader::feature::Value::String("park".to_string()),
        )]);

        let forest = HashMap::from([(
            "type".to_string(),
            mvt_reader::feature::Value::String("forest".to_string()),
        )]);

        let filter = Filter(vec![
            serde_json::Value::String("==".to_string()),
            serde_json::Value::String("type".to_string()),
            serde_json::Value::String("park".to_string()),
        ]);

        assert!(filter.matches(&park));
        assert!(!filter.matches(&forest));
    }
}
