//! Extra functionalities that can be used with the map.

mod kml;
mod labeled_symbol;
mod places;

pub use kml::{KmlFeature, KmlGeometry, KmlLayer, KmlStyle, KmlVisualDefaults, parse_kml};
pub use labeled_symbol::{
    LabeledSymbol, LabeledSymbolGroup, LabeledSymbolGroupStyle, LabeledSymbolStyle, Symbol,
};
pub use places::{Group, GroupedPlaces, GroupedPlacesTree, Place, Places};
