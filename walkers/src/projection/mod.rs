mod mercator;
mod projector;

pub use mercator::MercatorProjection;
pub use projector::Projector;

use crate::{Position, position::Pixels};

/// Turns geographical positions into pixels of the whole map at given zoom level, and back.
///
/// At zoom 0, the whole map is expected to fit in a single 256x256 tile, doubling with each zoom
/// level, so that tiles can be laid out on it.
pub trait Projection {
    /// Project geographical position into pixels.
    fn project(&self, position: Position, zoom: f64) -> Pixels;

    /// Get geographical position from pixels.
    fn unproject(&self, pixels: Pixels, zoom: f64) -> Position;

    /// What is the local scale of the map at the provided position and zoom level?
    fn scale_pixel_per_meter(&self, position: Position, zoom: f64) -> f64;
}
