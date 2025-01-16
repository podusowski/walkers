#![doc = include_str!("../README.md")]
#![deny(clippy::unwrap_used, rustdoc::broken_intra_doc_links)]

mod center;
mod download;
mod global_map;
mod io;
mod local_map;
mod map_memory;
mod projector;
pub mod sources;
mod tiles;
mod units;
mod zoom;

pub use download::{HeaderValue, HttpOptions};
pub use global_map::Map;
pub use local_map::LocalMap;

pub use map_memory::MapMemory;
pub use projector::*;
pub use tiles::{HttpTiles, Texture, TextureWithUv, Tiles};
pub use units::{Pixel, Position};
pub use zoom::InvalidZoom;

/// Plugins allow drawing custom shapes on the map. After implementing this trait for your type,
/// you can add it to the map with [`Map::with_plugin`]
pub trait Plugin<P: Projector> {
    /// Function called at each frame.
    ///
    /// The provided [`Ui`] has its [`Ui::max_rect`] set to the full rect that was allocated
    /// by the map widget. Implementations should typically use the provided [`Projector`] to
    /// compute target screen coordinates and use one of the various egui methods to draw at these
    /// coordinates instead of relying on [`Ui`] layout system.
    ///
    /// The provided [`Response`] is the response of the map widget itself and can be used to test
    /// if the mouse is hovering or clicking on the map.
    fn run(self: Box<Self>, ui: &mut egui::Ui, response: &egui::Response, projector: &P);
}
