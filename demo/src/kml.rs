use egui::Color32;
use walkers_extras::{KmlLayer, KmlVisualDefaults, parse_kml};

/// Poland borders
pub fn poland_borders() -> KmlLayer {
    let defaults = KmlVisualDefaults {
        polygon_fill_color: Color32::from_rgba_unmultiplied(0, 0, 0, 0),
        polygon_outline_color: Color32::from_rgb(0xFF, 0x00, 0x00),
        polygon_outline_width: 3.0,
        ..KmlVisualDefaults::default()
    };

    KmlLayer::from_string(include_str!("../assets/Poland.kml")).with_defaults(defaults)
}

/// Outdoor gyms Umeå
/// https://data.europa.eu/data/datasets/utegym-umea-opendata-umea-se
pub fn outgym_umea_layer() -> KmlLayer {
    let defaults = KmlVisualDefaults {
        polygon_fill_color: Color32::from_rgba_unmultiplied(0, 0, 0, 0),
        polygon_outline_color: Color32::from_rgb(0x00, 0xFF, 0x00),
        polygon_outline_width: 3.0,
        ..KmlVisualDefaults::default()
    };

    KmlLayer::from_string(include_str!("../assets/utegym-umea.kml")).with_defaults(defaults)
}
