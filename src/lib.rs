#![doc = include_str!("../README.md")]
#![deny(clippy::unwrap_used, rustdoc::broken_intra_doc_links)]

mod map;
mod mercator;
pub mod providers;

#[cfg(target_arch = "wasm32")]
mod tiles_wasm;

#[cfg(not(target_arch = "wasm32"))]
mod tiles;

mod tokio;
mod zoom;

pub use map::{Center, Map, MapMemory, Projector};
pub use mercator::{screen_to_position, Position, PositionExt};

#[cfg(not(target_arch = "wasm32"))]
pub use tiles::Tiles;

#[cfg(target_arch = "wasm32")]
pub use tiles_wasm::Tiles;

pub use zoom::Zoom;
