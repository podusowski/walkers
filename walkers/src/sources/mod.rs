//! Some common HTTP tile sources. Make sure you follow terms of usage of the particular source.

mod geoportal;
mod mapbox;
#[cfg(feature = "mvt")]
mod openfreemap;
mod openstreetmap;

use crate::TileId;
pub use geoportal::Geoportal;
pub use mapbox::{Mapbox, MapboxStyle};
#[cfg(feature = "mvt")]
pub use openfreemap::OpenFreeMap;
pub use openstreetmap::OpenStreetMap;

#[derive(Clone)]
pub struct Attribution {
    pub text: &'static str,
    pub url: &'static str,
    pub logo_light: Option<egui::ImageSource<'static>>,
    pub logo_dark: Option<egui::ImageSource<'static>>,
}

/// Remote tile server definition, source for the [`crate::HttpTiles`].
pub trait TileSource {
    fn tile_url(&self, tile_id: TileId) -> String;
    fn attribution(&self) -> Attribution;

    /// Size of each tile, in pixels. Walkers works with 256px tiles internally, so this
    /// should be 256 multiplied or divided by a power of two, for example 128, 256 or 512.
    fn tile_size(&self) -> u32 {
        256
    }

    fn max_zoom(&self) -> u8 {
        19
    }
}
