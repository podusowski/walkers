//! Types and functions for working with positions.

use crate::mercator::{project, unproject};
use egui::Vec2;

/// Geographical position with latitude and longitude.
pub type Position = geo_types::Point;

/// Construct `Position` from latitude and longitude.
pub fn lat_lon(lat: f64, lon: f64) -> Position {
    Position::new(lon, lat)
}

/// Construct `Position` from longitude and latitude. Note that it is common standard to write
/// coordinates starting with the latitude instead (e.g. `51.104465719934176, 17.075169894118684` is
/// the [Wrocław's zoo](https://zoo.wroclaw.pl/en/)).
pub fn lon_lat(lon: f64, lat: f64) -> Position {
    Position::new(lon, lat)
}

/// Geographical [`Position`] shifted by a number of pixels on the screen.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub struct AdjustedPosition {
    /// Base geographical position.
    pub position: Position,
    /// Offset in pixels.
    pub offset: Pixels,
    /// Zoom level at which the position was adjusted.
    pub zoom: f64,
}

impl AdjustedPosition {
    pub(crate) fn new(position: Position) -> Self {
        Self {
            position,
            offset: Pixels::new(0.0, 0.0),
            zoom: 1.0, // Does not matter, as offset is zero.
        }
    }

    /// Calculate the real position, i.e. including the offset.
    pub(crate) fn position(&self) -> Position {
        unproject(project(self.position, self.zoom) - self.offset, self.zoom)
    }

    /// Recalculate `position` so that `offset` is zero.
    pub(crate) fn zero_offset(self, zoom: f64) -> Self {
        Self {
            position: self.position(),
            offset: Default::default(),
            zoom,
        }
    }

    pub(crate) fn shift(self, offset: Vec2, zoom: f64) -> Self {
        Self {
            position: self.position(),
            offset: Pixels::new(offset.x as f64, offset.y as f64),
            zoom,
        }
    }
}

impl From<Position> for AdjustedPosition {
    fn from(position: Position) -> Self {
        Self {
            position,
            offset: Default::default(),
            zoom: 1.0, // TODO: THis is made up.
        }
    }
}

/// Location projected on the screen or an abstract bitmap.
pub type Pixels = geo_types::Point;

pub trait PixelsExt {
    fn to_vec2(&self) -> egui::Vec2;
}

impl PixelsExt for Pixels {
    fn to_vec2(&self) -> egui::Vec2 {
        egui::Vec2::new(self.x() as f32, self.y() as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adjusted_position() {
        let position = lat_lon(51.0, 17.0);

        let adjusted =
            AdjustedPosition::new(position).shift(Pixels::new(10.0, 20.0).to_vec2(), 10.0);
        approx::assert_relative_eq!(adjusted.position().x(), 16.98626708984377);
        approx::assert_relative_eq!(adjusted.position().y(), 51.017281581280216);

        // When zoom is lower, the offset expressed as screen pixels will be larger.
        let adjusted =
            AdjustedPosition::new(position).shift(Pixels::new(10.0, 20.0).to_vec2(), 2.0);
        approx::assert_relative_eq!(adjusted.position().x(), 13.48437500000002);
        approx::assert_relative_eq!(adjusted.position().y(), 55.21655462355652);

        let adjusted = AdjustedPosition::new(position)
            .shift(Pixels::new(10.0, 20.0).to_vec2(), 2.0)
            .shift(Pixels::new(0.0, 0.0).to_vec2(), 10.0);
        approx::assert_relative_eq!(adjusted.position().x(), 13.48437500000002);
        approx::assert_relative_eq!(adjusted.position().y(), 55.21655462355652);
    }
}
