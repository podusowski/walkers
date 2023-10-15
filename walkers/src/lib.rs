#![doc = include_str!("../README.md")]
#![deny(clippy::unwrap_used, rustdoc::broken_intra_doc_links)]

mod io;
mod map;
mod mercator;
pub mod providers;
mod tiles;
mod zoom;

pub use map::{Center, Map, MapMemory, Plugin, Projector};
pub use mercator::{screen_to_position, Position, PositionExt};
pub use tiles::Tiles;
pub use zoom::Zoom;
