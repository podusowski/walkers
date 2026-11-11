use std::sync::Arc;

use bytes::Bytes;

use crate::{Stats, TileId, sources::Attribution};

/// What a [`TiledSource`] gives for a tile, before any style is applied to it.
#[non_exhaustive]
pub enum SourceData {
    /// A raster image (PNG, JPEG or WebP), or a Mapbox Vector Tile if the `mvt` feature is
    /// enabled. Which one it is, is told by the data itself.
    Encoded(Bytes),

    /// A raster image whose pixels are elevation, packed as `encoding` says. A `hillshade` style
    /// layer draws it.
    Elevation { data: Bytes, encoding: DemEncoding },
}

/// What a [`TiledSource`] has for a tile.
#[derive(Clone)]
pub enum TileData {
    Ready(Arc<SourceData>),

    /// Being fetched, or about to be.
    Loading,

    /// There is no such tile, so the one of a lower zoom level covering it is drawn instead.
    Missing,
}

/// How elevation is packed into the pixels of a raster image.
///
/// <https://maplibre.org/maplibre-style-spec/sources/#encoding>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemEncoding {
    /// Mapbox Terrain-RGB.
    Mapbox,
    Terrarium,
}

/// Source of tiles' data, such as a tile server or a PMTiles file. It knows nothing about how the
/// data is drawn, that is up to the style of [`crate::StyledSources`].
pub trait TiledSource {
    /// Data of that exact tile. Unless it is ready or missing, it is fetched in the background.
    fn data(&mut self, tile_id: TileId) -> TileData;

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
