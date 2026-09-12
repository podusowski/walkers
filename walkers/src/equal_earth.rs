//! Spherical Equal Earth projection.
//!
//! The equations are from B. Šavrič, T. Patterson, and B. Jenny (2018),
//! ["The Equal Earth map projection"](https://doi.org/10.1080/13658816.2018.1504949).

use std::f64::consts::PI;

use crate::{Position, lon_lat, position::Pixels};

const A1: f64 = 1.340_264;
const A2: f64 = -0.081_106;
const A3: f64 = 0.000_893;
const A4: f64 = 0.003_796;
const M: f64 = 0.866_025_403_784_438_6; // sqrt(3) / 2
const MAX_Y: f64 = 1.317_362_759_157_413_3;
const INVERSE_EPSILON: f64 = 1e-11;
const MAX_INVERSE_ITERATIONS: usize = 12;

/// Maximum x coordinate on a unit sphere, reached at longitude 180° on the equator.
pub(crate) const MAX_X: f64 = PI / (M * A1);

/// Project longitude and latitude in degrees onto a unit sphere.
pub(crate) fn project(position: Position) -> Pixels {
    let longitude = position.x().to_radians();
    let latitude = position.y().to_radians();
    let theta = (M * latitude.sin()).asin();
    let theta_squared = theta * theta;
    let theta_sixth = theta_squared * theta_squared * theta_squared;
    let derivative =
        A1 + 3.0 * A2 * theta_squared + theta_sixth * (7.0 * A3 + 9.0 * A4 * theta_squared);

    let x = longitude * theta.cos() / (M * derivative);
    let y = theta * (A1 + A2 * theta_squared + theta_sixth * (A3 + A4 * theta_squared));

    Pixels::new(x, y)
}

/// Convert Equal Earth coordinates on a unit sphere to longitude and latitude in degrees.
pub(crate) fn unproject(projected: Pixels) -> Position {
    let y = projected.y().clamp(-MAX_Y, MAX_Y);
    let mut theta = y;

    for _ in 0..MAX_INVERSE_ITERATIONS {
        let theta_squared = theta * theta;
        let theta_sixth = theta_squared * theta_squared * theta_squared;
        let polynomial = A1 + A2 * theta_squared + theta_sixth * (A3 + A4 * theta_squared);
        let derivative =
            A1 + 3.0 * A2 * theta_squared + theta_sixth * (7.0 * A3 + 9.0 * A4 * theta_squared);
        let correction = (theta * polynomial - y) / derivative;
        theta -= correction;

        if correction.abs() < INVERSE_EPSILON {
            break;
        }
    }

    let theta_squared = theta * theta;
    let theta_sixth = theta_squared * theta_squared * theta_squared;
    let derivative =
        A1 + 3.0 * A2 * theta_squared + theta_sixth * (7.0 * A3 + 9.0 * A4 * theta_squared);
    let longitude = M * projected.x() * derivative / theta.cos();
    let latitude = (theta.sin() / M).clamp(-1.0, 1.0).asin();

    lon_lat(longitude.to_degrees(), latitude.to_degrees())
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn forward_matches_proj_reference_values() {
        let projected = project(lon_lat(122.0, 47.0));

        assert_abs_diff_eq!(projected.x(), 1.549_254_331_329_069, epsilon = 1e-14);
        assert_abs_diff_eq!(projected.y(), 0.893_308_324_948_093, epsilon = 1e-14);
    }

    #[test]
    fn project_and_unproject_are_inverses() {
        for original in [
            lon_lat(-180.0, -90.0),
            lon_lat(-122.0, 47.0),
            lon_lat(0.0, 0.0),
            lon_lat(21.0, 52.0),
            lon_lat(180.0, 90.0),
        ] {
            let unprojected = unproject(project(original));

            assert_abs_diff_eq!(unprojected.x(), original.x(), epsilon = 1e-10);
            assert_abs_diff_eq!(unprojected.y(), original.y(), epsilon = 1e-10);
        }
    }
}
