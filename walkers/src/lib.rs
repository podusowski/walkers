#![doc = include_str!("../README.md")]
#![deny(clippy::unwrap_used, rustdoc::broken_intra_doc_links)]

mod center;
mod http_tiles;
mod io;
mod local_tiles;
mod map;
mod memory;
mod style;

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
pub use style::{Color, Filter, Layer, Style};
pub use tiles::{Tile, TileId, TilePiece, Tiles};
pub use zoom::InvalidZoom;
