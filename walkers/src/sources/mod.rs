//! Some common HTTP tile sources. Make sure you follow terms of usage of the particular source.

mod geoportal;
mod mapbox;
#[cfg(feature = "mvt")]
mod openfreemap;
mod openstreetmap;
mod opentopomap;

use crate::TileId;
use crate::projector::Projection;
pub use geoportal::Geoportal;
pub use mapbox::{Mapbox, MapboxStyle};
#[cfg(feature = "mvt")]
pub use openfreemap::OpenFreeMap;
pub use openstreetmap::OpenStreetMap;
pub use opentopomap::{OpenTopoMap, OpenTopoServer};

#[derive(Clone)]
pub struct Attribution {
    pub text: &'static str,
    pub url: &'static str,
    pub logo_light: Option<egui::ImageSource<'static>>,
    pub logo_dark: Option<egui::ImageSource<'static>>,
}

/// Remote tile server definition, source for the [`crate::HttpTiles`].
pub trait TileSource {
    /// The projection this tile source uses.
    type Projection: Projection;

    fn tile_url(&self, tile_id: TileId) -> String;
    fn attribution(&self) -> Attribution;
    fn projection(&self) -> Self::Projection;

    /// Size of each tile, should be a multiple of 256.
    fn tile_size(&self) -> u32 {
        256
    }

    fn max_zoom(&self) -> u8 {
        19
    }
}
