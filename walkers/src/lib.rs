#![doc = include_str!("../README.md")]
#![deny(clippy::unwrap_used, rustdoc::broken_intra_doc_links)]

mod center;
mod download;
pub mod extras;
mod http_tiles;
mod io;
mod local_tiles;
mod map;
mod memory;
mod mercator;
mod position;
mod projector;
pub mod sources;
mod tiles;
mod zoom;
mod pmtiles;

pub use download::{HeaderValue, HttpOptions, MaxParallelDownloads};
pub use http_tiles::{HttpStats, HttpTiles};
pub use local_tiles::LocalTiles;
pub use map::{Map, Plugin};
pub use memory::MapMemory;
pub use position::{lat_lon, lon_lat, Position};
pub use projector::Projector;
pub use tiles::{Texture, TextureWithUv, TileId, Tiles};
pub use zoom::InvalidZoom;
