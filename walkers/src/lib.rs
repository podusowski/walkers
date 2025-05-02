#![doc = include_str!("../README.md")]
#![deny(clippy::unwrap_used, rustdoc::broken_intra_doc_links)]

mod center;
mod download;
pub mod extras;
mod http_tiles;
mod io;
mod map;
mod mercator;
mod position;
pub mod sources;
mod tiles;
mod zoom;

pub use download::{HeaderValue, HttpOptions, MaxParallelDownloads};
pub use http_tiles::{HttpStats, HttpTiles};
pub use map::{Map, MapMemory, Plugin, Projector};
pub use position::{lat_lon, lon_lat, Position};
pub use tiles::{Texture, TextureWithUv, TileId, Tiles};
pub use zoom::InvalidZoom;
