//! Extra functionalities that can be used with the map.

mod image;
mod labeled_symbol;
mod places;
pub use crate::tiles::Texture;
pub use image::Image;
pub use labeled_symbol::{LabeledSymbol, LabeledSymbolStyle};
pub use places::{Group, GroupedPlace, GroupedPlaces, Place, Places};
