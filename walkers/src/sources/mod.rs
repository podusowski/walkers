//! Some common HTTP tile sources. Make sure you follow terms of usage of the particular source.

mod geoportal;
mod mapbox;
mod openstreetmap;

use crate::mercator::TileId;
pub use geoportal::Geoportal;
pub use mapbox::{Mapbox, MapboxStyle};
pub use openstreetmap::OpenStreetMap;

#[derive(Clone)]
/// Attribution information for the tile source.
pub struct Attribution {
    /// Attribution text.
    pub text: &'static str,
    /// URL to the attribution source.
    pub url: &'static str,
    /// Logo for the attribution.
    pub logo_light: Option<egui::ImageSource<'static>>,
    /// Dark version of the logo.
    pub logo_dark: Option<egui::ImageSource<'static>>,
}

/// Remote tile server definition, source for the [`crate::HttpTiles`].
pub trait TileSource {
    /// URL for the tile with the given id.
    fn tile_url(&self, tile_id: TileId) -> String;
    /// Attribution information for the tile source.
    fn attribution(&self) -> Attribution;

    /// Size of each tile, should be a multiple of 256.
    fn tile_size(&self) -> u32 {
        256
    }

    /// Maximum zoom level.
    fn max_zoom(&self) -> u8 {
        19
    }
}
