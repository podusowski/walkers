use walkers_extras::KmlLayer;

/// Poland borders
pub fn poland_borders() -> KmlLayer {
    KmlLayer::from_string(include_str!("../assets/Poland.kml"))
}

/// Outdoor gyms Umeå
/// https://data.europa.eu/data/datasets/utegym-umea-opendata-umea-se
pub fn outgym_umea_layer() -> KmlLayer {
    KmlLayer::from_string(include_str!("../assets/utegym-umea.kml"))
}
