#![doc = include_str!("../README.md")]
#![deny(clippy::unwrap_used, rustdoc::broken_intra_doc_links)]

mod center;
mod download;
pub mod extras;
mod io;
mod map;
mod mercator;
mod position;
pub mod sources;
mod tiles;
mod zoom;

pub use download::{HeaderValue, HttpOptions};
pub use map::{Map, MapMemory, Plugin, Projector};
pub use mercator::{screen_to_position, TileId};
pub use position::{lat_lon, lon_lat, Position};
pub use tiles::{HttpTiles, Texture, TextureWithUv, Tiles};
pub use zoom::InvalidZoom;
