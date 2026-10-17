//! Types and functions for working with positions.

use crate::Projection;
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

/// Move `position` by `offset` pixels on the screen, the way dragging the map at `zoom` does.
pub(crate) fn shifted(
    projection: &dyn Projection,
    position: Position,
    offset: Vec2,
    zoom: f64,
) -> Position {
    projection.unproject(
        projection.project(position, zoom) - Pixels::from_vec2(offset),
        zoom,
    )
}

/// Position halfway between `a` and `b` on the screen.
pub(crate) fn halfway(
    projection: &dyn Projection,
    a: Position,
    b: Position,
    zoom: f64,
) -> Position {
    projection.unproject(
        (projection.project(a, zoom) + projection.project(b, zoom)) / 2.,
        zoom,
    )
}

/// Distance between two positions on the screen, in pixels.
pub(crate) fn pixel_distance(
    projection: &dyn Projection,
    a: Position,
    b: Position,
    zoom: f64,
) -> f32 {
    (projection.project(a, zoom) - projection.project(b, zoom))
        .to_vec2()
        .length()
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
    use crate::MercatorProjection;
    use approx::assert_relative_eq;

    fn base_position() -> Position {
        lat_lon(51.0, 17.0)
    }

    #[test]
    fn shifting_position() {
        let position = shifted(
            &MercatorProjection,
            base_position(),
            Vec2::new(10.0, 20.0),
            10.0,
        );
        assert_relative_eq!(position.x(), 16.98626708984377);
        assert_relative_eq!(position.y(), 51.017281581280216);

        // When zoom is lower, the offset expressed as screen pixels will be larger.
        let position = shifted(
            &MercatorProjection,
            base_position(),
            Vec2::new(10.0, 20.0),
            2.0,
        );
        assert_relative_eq!(position.x(), 13.48437500000002);
        assert_relative_eq!(position.y(), 55.21655462355652);
    }

    #[test]
    fn shifting_position_by_nothing() {
        // Projecting there and back is not exact, but off by nanometers.
        let position = shifted(&MercatorProjection, base_position(), Vec2::ZERO, 10.0);
        assert_relative_eq!(position.x(), base_position().x(), epsilon = 1e-9);
        assert_relative_eq!(position.y(), base_position().y(), epsilon = 1e-9);
    }

    #[test]
    fn shifting_position_using_different_zoom() {
        let position = shifted(
            &MercatorProjection,
            base_position(),
            Vec2::new(5.0, 10.0),
            10.0,
        );
        let position = shifted(&MercatorProjection, position, Vec2::new(10.0, 20.0), 11.0);

        assert_relative_eq!(position.x(), 16.98626708984377);
        assert_relative_eq!(position.y(), 51.017281581280216);
    }

    #[test]
    fn pixel_distance_between_shifted_positions() {
        let position = shifted(
            &MercatorProjection,
            base_position(),
            Vec2::new(10.0, 0.0),
            10.0,
        );
        assert_relative_eq!(
            pixel_distance(&MercatorProjection, position, base_position(), 10.0),
            10.0
        );

        // Shifting it further should increase the distance.
        let position = shifted(&MercatorProjection, position, Vec2::new(10.0, 0.0), 10.0);
        assert_relative_eq!(
            pixel_distance(&MercatorProjection, position, base_position(), 10.0),
            20.0
        );
    }

    #[test]
    fn halfway_between_positions() {
        let far = shifted(
            &MercatorProjection,
            base_position(),
            Vec2::new(10.0, 20.0),
            10.0,
        );
        let middle = halfway(&MercatorProjection, far, base_position(), 10.0);
        let expected = shifted(
            &MercatorProjection,
            base_position(),
            Vec2::new(5.0, 10.0),
            10.0,
        );
        assert_relative_eq!(middle.x(), expected.x());
        assert_relative_eq!(middle.y(), expected.y());
    }
}
