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

/// [`Position`] alone is not able to represent detached (e.g. after map gets dragged) position
/// due to insufficient accuracy.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub struct AdjustedPosition {
    /// Base geographical position.
    pub position: Position,

    /// Offset in pixels.
    pub offset: Pixels,
}

impl AdjustedPosition {
    pub(crate) fn new(position: Position, offset: Pixels) -> Self {
        Self { position, offset }
    }

    /// Calculate the real position, i.e. including the offset.
    pub(crate) fn position(&self, zoom: f64) -> Position {
        unproject(project(self.position, zoom) - self.offset, zoom)
    }

    /// Recalculate `position` so that `offset` is zero.
    pub(crate) fn zero_offset(self, zoom: f64) -> Self {
        Self {
            position: self.position(zoom),
            offset: Default::default(),
        }
    }

    pub(crate) fn shift(self, offset: Vec2) -> Self {
        Self {
            position: self.position,
            offset: self.offset + Pixels::new(offset.x as f64, offset.y as f64),
        }
    }
}

impl From<Position> for AdjustedPosition {
    fn from(position: Position) -> Self {
        Self {
            position,
            offset: Default::default(),
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
