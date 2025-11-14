#![doc = include_str!("../README.md")]
#![deny(clippy::unwrap_used, rustdoc::broken_intra_doc_links)]

mod center;
mod download;
mod http_tiles;
mod io;
mod loader;
mod local_tiles;
mod map;
mod memory;

// TODO: I don't want it to be public.
pub mod mercator;

#[cfg(feature = "vector_tiles")]
mod mvt;
#[cfg(feature = "vector_tiles")]
mod pmtiles;
mod position;
mod projector;
pub mod sources;
mod tiles;
mod zoom;

pub use download::{HeaderValue, HttpOptions, MaxParallelDownloads};
pub use http_tiles::{HttpStats, HttpTiles};
pub use local_tiles::LocalTiles;
pub use map::{Map, Plugin};
pub use memory::MapMemory;
#[cfg(feature = "vector_tiles")]
pub use pmtiles::PmTiles;
pub use position::{Position, lat_lon, lon_lat};
pub use projector::Projector;
pub use tiles::{Tile, TextureWithUv, TileId, Tiles};
pub use zoom::InvalidZoom;
