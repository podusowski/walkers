use walkers::{Color, Float, Layer, Paint, Style, json};
use walkers_extras::KmlLayer;

/// Kampinos National Park, from OpenStreetMap.
pub fn kampinos_national_park() -> KmlLayer {
    let style = Style {
        layers: vec![Layer::Fill {
            source_layer: "park".into(),
            filter: None,
            paint: Paint {
                fill_color: Some(Color(json!("#2e8b57"))),
                fill_opacity: Some(Float(json!(0.4))),
                ..Default::default()
            },
        }],
    };

    KmlLayer::from_string(include_str!("../assets/kampinos-national-park.kml"), style)
}

/// Planned high-speed railways in Poland, from OpenStreetMap.
pub fn high_speed_rail_poland() -> KmlLayer {
    let style = Style {
        layers: vec![Layer::Line {
            source_layer: "railways".into(),
            filter: None,
            paint: Paint {
                line_color: Some(Color(json!("#8000c0"))),
                line_width: Some(Float(json!(3.0))),
                ..Default::default()
            },
        }],
    };

    KmlLayer::from_string(include_str!("../assets/high-speed-rail-poland.kml"), style)
}

/// Outdoor gyms Umeå
/// https://data.europa.eu/data/datasets/utegym-umea-opendata-umea-se
pub fn outgym_umea_layer() -> KmlLayer {
    let style = Style {
        layers: vec![Layer::Circle {
            source_layer: "".into(),
            filter: None,
        }],
    };

    KmlLayer::from_string(include_str!("../assets/utegym-umea.kml"), style)
}
