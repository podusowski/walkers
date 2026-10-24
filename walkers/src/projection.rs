use crate::{Position, mercator, position::Pixels};

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

/// [Web Mercator](https://en.wikipedia.org/wiki/Web_Mercator_projection), used by most of the
/// tile sources and default in [`crate::Map`].
#[derive(Debug, Clone, Copy, Default)]
pub struct MercatorProjection;

impl Projection for MercatorProjection {
    fn project(&self, position: Position, zoom: f64) -> Pixels {
        mercator::project(position, zoom)
    }

    fn unproject(&self, pixels: Pixels, zoom: f64) -> Position {
        mercator::unproject(pixels, zoom)
    }

    fn scale_pixel_per_meter(&self, position: Position, zoom: f64) -> f64 {
        mercator::pixels_per_meter(position.y(), zoom)
    }
}
