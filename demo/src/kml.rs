use walkers::{Color, Float, Layer, Paint, Style, json};
use walkers_extras::KmlLayer;

/// Poland borders
pub fn poland_borders() -> KmlLayer {
    let style = Style {
        layers: vec![Layer::Line {
            // TODO: Actually, it does not matter.
            source_layer: "borders".to_string(),
            filter: None,
            paint: Paint {
                line_color: Some(Color(json!("#ff0000"))),
                line_width: Some(Float(json!(2.0))),
                ..Default::default()
            },
        }],
    };

    KmlLayer::from_string(include_str!("../assets/Poland.kml"), style)
}

/// Outdoor gyms Umeå
/// https://data.europa.eu/data/datasets/utegym-umea-opendata-umea-se
pub fn outgym_umea_layer() -> KmlLayer {
    let style = Style {
        layers: vec![Layer::Circle {
            source_layer: "".to_string(),
            filter: None,
        }],
    };

    KmlLayer::from_string(include_str!("../assets/utegym-umea.kml"), style)
}
