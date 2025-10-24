//! Extra functionalities that can be used with the map.

mod labeled_symbol;
mod places;
pub use labeled_symbol::{
    LabeledSymbol, LabeledSymbolGroup, LabeledSymbolGroupStyle, LabeledSymbolStyle, Symbol,
};
pub use places::{Group, GroupedPlaces, Place, Places};
pub use walkers::Texture;
pub use places::GroupedPlacesTree;
