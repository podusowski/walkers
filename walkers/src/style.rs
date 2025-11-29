/// Style for rendering vector maps. Loosely (very) based on MapLibre's style specification.
#[derive(serde::Deserialize)]
struct Style {
    layers: Vec<StyleLayer>,
}

#[derive(serde::Deserialize)]
struct StyleLayer {
    source_layer: String,
    filter: Vec<String>,
    paint: Paint,
}

#[derive(serde::Deserialize)]
struct Paint {
    fill_color: color::DynamicColor,
}

mod tests {
    use std::env;

    use super::*;

    #[test]
    fn test_style_parsing() {
        let style = std::fs::read_to_string(
            env!("CARGO_MANIFEST_DIR").to_owned() + "/assets/protonmaps-dark-style.json",
        )
        .unwrap();

        let _parsed_style: Style = serde_json::from_str(&style).unwrap();
    }
}
