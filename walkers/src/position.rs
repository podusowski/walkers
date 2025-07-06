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
    pub fn new(position: Position) -> Self {
        Self {
            position,
            offset: Pixels::new(0.0, 0.0),
            zoom: 1.0, // Does not matter, as offset is zero.
        }
    }

    /// Calculate the real position, i.e. including the offset.
    pub fn position(&self) -> Position {
        unproject(project(self.position, self.zoom) - self.offset, self.zoom)
    }

    pub fn shift(self, offset: Vec2, zoom: f64) -> Self {
        let changed_zoom_factor = 2.0_f64.powf(zoom - self.zoom);
        Self {
            position: self.position,
            offset: self.offset * changed_zoom_factor + Pixels::from_vec2(offset),
            zoom,
        }
    }

    pub fn offset_length(&self) -> f32 {
        self.offset.to_vec2().length()
    }
}

/// Location projected on the screen or an abstract bitmap.
pub type Pixels = geo_types::Point;

pub trait PixelsExt {
    fn to_vec2(&self) -> egui::Vec2;
    fn from_vec2(_: egui::Vec2) -> Self;
}

impl PixelsExt for Pixels {
    fn to_vec2(&self) -> egui::Vec2 {
        egui::Vec2::new(self.x() as f32, self.y() as f32)
    }

    fn from_vec2(vec2: egui::Vec2) -> Self {
        Pixels::new(vec2.x as f64, vec2.y as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn base_adjusted_position() -> AdjustedPosition {
        AdjustedPosition::new(lat_lon(51.0, 17.0))
    }

    #[test]
    fn shifting_adjusted_position() {
        let position = base_adjusted_position().shift(Pixels::new(10.0, 20.0).to_vec2(), 10.0);
        approx::assert_relative_eq!(position.position().x(), 16.98626708984377);
        approx::assert_relative_eq!(position.position().y(), 51.017281581280216);

        // When zoom is lower, the offset expressed as screen pixels will be larger.
        let position = base_adjusted_position().shift(Pixels::new(10.0, 20.0).to_vec2(), 2.0);
        approx::assert_relative_eq!(position.position().x(), 13.48437500000002);
        approx::assert_relative_eq!(position.position().y(), 55.21655462355652);
    }

    #[test]
    fn shifting_adjusted_position_by_nothing() {
        let position = base_adjusted_position()
            .shift(Pixels::new(10.0, 20.0).to_vec2(), 2.0)
            .shift(Pixels::new(0.0, 0.0).to_vec2(), 10.0);
        approx::assert_relative_eq!(position.position().x(), 13.48437500000002);
        approx::assert_relative_eq!(position.position().y(), 55.21655462355652);
    }

    #[test]
    fn shifting_adjusted_position_using_different_zoom() {
        let position = base_adjusted_position()
            .shift(Pixels::new(5.0, 10.0).to_vec2(), 10.0)
            .shift(Pixels::new(10.0, 20.0).to_vec2(), 11.0);

        approx::assert_relative_eq!(position.position().x(), 16.98626708984377);
        approx::assert_relative_eq!(position.position().y(), 51.017281581280216);
    }

    #[test]
    fn test_adjusted_position_offset_length() {
        let position = base_adjusted_position().shift(Pixels::new(10.0, 0.0).to_vec2(), 10.0);
        assert_relative_eq!(position.offset_length(), 10.0);

        // Shifting it further should increase the offset length.
        let position = position.shift(Pixels::new(10.0, 0.0).to_vec2(), 10.0);
        assert_relative_eq!(position.offset_length(), 20.0);
    }
}
