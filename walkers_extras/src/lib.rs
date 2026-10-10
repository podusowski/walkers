//! Extra functionalities that can be used with the map.

mod basemaps;
mod geojson;
mod kml;
mod labeled_symbol;
mod places;

pub use basemaps::{Basemaps, pmtiles_files};
pub use geojson::GeoJsonLayer;
pub use kml::KmlLayer;
pub use labeled_symbol::{
    LabeledSymbol, LabeledSymbolGroup, LabeledSymbolGroupStyle, LabeledSymbolStyle, Symbol,
};
pub use places::{Group, GroupedPlaces, GroupedPlacesTree, Place, Places};
