#![doc = include_str!("../README.md")]
#![deny(clippy::unwrap_used, rustdoc::broken_intra_doc_links)]

mod canvas;
mod center;
mod io;
mod layer;
mod map;
mod memory;
mod options;
mod plugin;
#[cfg(feature = "vector")]
mod style;
#[cfg(feature = "vector")]
mod text;

#[cfg(not(feature = "vector"))]
mod style {
    /// Dummy style, used when `vector` feature is not enabled.
    #[derive(Default)]
    pub struct Style;
}

#[cfg(feature = "vector")]
mod egui_backend;

#[cfg(feature = "vector")]
mod expression;
mod position;
mod projection;
#[cfg(feature = "vector")]
mod render;
pub mod sources;
mod tiles;
mod z_order;
mod zoom;

pub use canvas::Canvas;
#[cfg(feature = "vector")]
pub use canvas::Drawables;
#[cfg(feature = "vector")]
pub use egui_backend::wgpu::install_renderer;
#[cfg(feature = "vector")]
pub use expression::Context;
pub use geo_types;
pub use io::tiles_io::Stats;
pub use io::{HeaderValue, MaxParallelDownloads, http::HttpOptions};
pub use layer::Layer;
pub use map::Map;
pub use memory::MapMemory;
pub use options::Options;
pub use plugin::Plugin;
pub use position::{Pixels, Position, lat_lon, lon_lat};
pub use projection::{MercatorProjection, Projection, Projector};
#[cfg(feature = "vector")]
pub use render::drawable::{Drawable, Line, Mesh, Vertex};
#[cfg(feature = "vector")]
pub use render::{
    fill::render as render_fill, fill::tessellate_polygon, line::render as render_line,
    symbol::render as render_symbol,
};
pub use style::Style;
#[cfg(feature = "vector")]
pub use style::{
    Color, Dasharray, Filter, Float, Layout, Paint, SourceLayer, StyleLayer, Value,
    exponential_zoom_interpolation, json, linear_zoom_interpolation,
};
#[cfg(feature = "vector")]
pub use text::{Placement, Text, place_texts};
pub use tiles::http::HttpTiles;
#[cfg(feature = "pmtiles")]
pub use tiles::pmtiles::PmTiles;
pub use tiles::{Tile, TileId, TilePiece, Tiles};
pub use z_order::ZOrder;
pub use zoom::InvalidZoom;
