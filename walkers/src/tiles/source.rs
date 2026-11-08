use std::sync::Arc;

use bytes::Bytes;

use crate::{Stats, TileId, sources::Attribution};

/// What a [`TiledSource`] gives for a tile, before any style is applied to it.
#[non_exhaustive]
pub enum SourceData {
    /// A raster image (PNG or JPEG), or a Mapbox Vector Tile if the `mvt` feature is enabled.
    /// Which one it is, is told by the data itself.
    Encoded(Bytes),
}

/// Source of tiles' data, such as a tile server or a PMTiles file. It knows nothing about how the
/// data is drawn, that is up to the style of [`crate::StyledSources`].
pub trait TiledSource {
    /// Data of that exact tile, if it is ready. Otherwise `None`, and it is fetched in the
    /// background, unless fetching it already failed.
    fn data(&mut self, tile_id: TileId) -> Option<Arc<SourceData>>;

    fn attribution(&self) -> Attribution;

    /// Size of each tile, in pixels. Walkers works with 256px tiles internally, so this
    /// should be 256 multiplied or divided by a power of two, for example 128, 256 or 512.
    fn tile_size(&self) -> u32 {
        256
    }

    /// Tiles past it are drawn from the ones at this zoom level.
    fn max_zoom(&self) -> u8 {
        19
    }

    /// Tiles being loaded right now.
    fn stats(&self) -> Stats {
        Stats::default()
    }
}
