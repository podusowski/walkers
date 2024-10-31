//! Some common HTTP tile sources. Make sure you follow terms of usage of the particular source.

mod geoportal;
mod mapbox;
mod openstreetmap;

use crate::mercator::TileId;
pub use geoportal::Geoportal;
pub use mapbox::{Mapbox, MapboxStyle};
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

    /// Size of each tile, should be a multiple of 256.
    fn tile_size(&self) -> u32 {
        256
    }

    fn max_zoom(&self) -> u8 {
        19
    }
}
