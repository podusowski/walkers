use walkers::{Color, Float, Layer, Paint, Style, json};
use walkers_extras::KmlLayer;

/// Poland borders
#[expect(clippy::expect_used)]
pub fn poland_borders() -> KmlLayer {
    let style = Style {
        layers: vec![Layer::Line {
            // TODO: Actually, it does not matter.
            source_layer: "borders".to_owned(),
            filter: None,
            paint: Paint {
                line_color: Some(Color(json!("#ff0000"))),
                line_width: Some(Float(json!(2.0))),
                ..Default::default()
            },
        }],
    };

    KmlLayer::from_string(include_str!("../assets/Poland.kml"), style)
        .expect("failed to parse Poland.kml")
}

/// Outdoor gyms Umeå
/// <https://data.europa.eu/data/datasets/utegym-umea-opendata-umea-se>
#[expect(clippy::expect_used)]
pub fn outgym_umea_layer() -> KmlLayer {
    let style = Style {
        layers: vec![Layer::Circle {
            source_layer: String::new(),
            filter: None,
        }],
    };

    KmlLayer::from_string(include_str!("../assets/utegym-umea.kml"), style)
        .expect("failed to parse utegym-umea.kml")
}
