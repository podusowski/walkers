//! Project the lat/lon coordinates into a 2D x/y using the Web Mercator.
//! <https://en.wikipedia.org/wiki/Web_Mercator_projection>

use crate::{
    Projection, lon_lat,
    position::{Pixels, Position},
    tile_grid::total_pixels,
};
use std::f64::consts::PI;

// zoom level   tile coverage  number of tiles  tile size(*) in degrees
// 0            1 tile         1 tile           360° x 170.1022°
// 1            2 × 2 tiles    4 tiles          180° x 85.0511°
// 2            4 × 4 tiles    16 tiles         90° x [variable]

/// [Web Mercator](https://en.wikipedia.org/wiki/Web_Mercator_projection), used by most of the
/// tile sources and default in [`crate::Map`].
#[derive(Debug, Clone, Copy, Default)]
pub struct MercatorProjection;

impl Projection for MercatorProjection {
    fn project(&self, position: Position, zoom: f64) -> Pixels {
        project(position, zoom)
    }

    fn unproject(&self, pixels: Pixels, zoom: f64) -> Position {
        unproject(pixels, zoom)
    }

    fn scale_pixel_per_meter(&self, position: Position, zoom: f64) -> f64 {
        pixels_per_meter(position.y(), zoom)
    }
}

/// Project the position into the Mercator projection and normalize it to 0-1 range.
fn mercator_normalized(position: Position) -> (f64, f64) {
    // Project into Mercator (cylindrical map projection).
    let x = position.x().to_radians();
    let y = position.y().to_radians().tan().asinh();

    // Scale both x and y to 0-1 range.
    let x = (1. + (x / PI)) / 2.;
    let y = (1. - (y / PI)) / 2.;

    (x, y)
}

/// Project geographical position into a 2D plane using Mercator.
fn project(position: Position, zoom: f64) -> Pixels {
    let total_pixels = total_pixels(zoom);
    let (x, y) = mercator_normalized(position);
    Pixels::new(x * total_pixels, y * total_pixels)
}

/// How many pixels there are in a meter at the given latitude and zoom level.
fn pixels_per_meter(latitude: f64, zoom: f64) -> f64 {
    const EARTH_CIRCUMFERENCE: f64 = 40_075_016.686;

    // Number of pixels for width of world at this zoom level
    let total_pixels = total_pixels(zoom);

    let pixel_per_meter_equator = total_pixels / EARTH_CIRCUMFERENCE;
    let latitude_rad = latitude.abs().to_radians();
    pixel_per_meter_equator / latitude_rad.cos()
}

/// Transforms screen pixels into a geographical position.
fn unproject(pixels: Pixels, zoom: f64) -> Position {
    let number_of_pixels = total_pixels(zoom);

    let lon = pixels.x();
    let lon = lon / number_of_pixels;
    let lon = (lon * 2. - 1.) * PI;
    let lon = lon.to_degrees();

    let lat = pixels.y();
    let lat = lat / number_of_pixels;
    let lat = (-lat * 2. + 1.) * PI;
    let lat = lat.sinh().atan().to_degrees();

    lon_lat(lon, lat)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lat_lon;
    use approx::assert_abs_diff_eq;

    #[test]
    fn test_equator_zoom_0() {
        // At zoom 0 (whole world), equator should be about 156.5km per pixel
        let scale = pixels_per_meter(0.0, 0.);
        assert_abs_diff_eq!(scale, 1. / 156_543.03, epsilon = 0.01);
    }

    #[test]
    fn test_equator_zoom_19() {
        // At max zoom (19), equator should be about 0.3m per pixel
        let scale = pixels_per_meter(0.0, 19.);
        assert_abs_diff_eq!(scale, 1. / 0.298, epsilon = 0.01);
    }

    #[test]
    fn project_there_and_back() {
        let citadel = lat_lon(21.00027, 52.26470);
        let zoom = 16;
        let calculated = unproject(project(citadel, zoom as f64), zoom as f64);

        approx::assert_relative_eq!(calculated.x(), citadel.x(), max_relative = 1.0);
        approx::assert_relative_eq!(calculated.y(), citadel.y(), max_relative = 1.0);
    }
}
