#![doc = include_str!("../README.md")]
#![deny(clippy::unwrap_used, rustdoc::broken_intra_doc_links)]

mod center;
mod http_tiles;
mod io;
mod local_tiles;
mod map;
mod memory;
#[cfg(feature = "mvt")]
mod style;

#[cfg(not(feature = "mvt"))]
mod style {
    /// Dummy style, used when `mtv` feature is not enabled.
    #[derive(Default)]
    pub struct Style;
}

// TODO: I don't want it to be public.
pub mod mercator;

#[cfg(feature = "mvt")]
mod expression;
#[cfg(feature = "mvt")]
mod mvt;
#[cfg(feature = "pmtiles")]
mod pmtiles;
mod position;
mod projector;
pub mod sources;
mod tiles;
mod zoom;

// TODO: In future, I'd like to expose full drawing API instead of this.
#[cfg(feature = "mvt")]
pub use mvt::tessellate_polygon;

pub use http_tiles::HttpTiles;
pub use io::tiles_io::Stats;
pub use io::{HeaderValue, MaxParallelDownloads, http::HttpOptions};
pub use local_tiles::LocalTiles;
pub use map::{Map, Plugin};
pub use memory::MapMemory;
#[cfg(feature = "pmtiles")]
pub use pmtiles::PmTiles;
pub use position::{Position, lat_lon, lon_lat};
pub use projector::Projector;
pub use style::Style;
#[cfg(feature = "mvt")]
pub use style::{Color, Filter, Layer};
pub use tiles::{Tile, TileId, TilePiece, Tiles};
pub use zoom::InvalidZoom;
