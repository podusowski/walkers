//! Project the lat/lon coordinates into a 2D x/y using the Web Mercator.
//! https://en.wikipedia.org/wiki/Web_Mercator_projection
//! https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames
//! https://www.netzwolf.info/osm/tilebrowser.html?lat=51.157800&lon=6.865500&zoom=14

/// Geographical position with latitude and longitude.
pub type Position = geo_types::Point;

/// Location projected on the screen or an abstract bitmap.
pub type Pixels = Pos2;

use egui::{Pos2, Vec2};
use std::f64::consts::PI;

pub trait PositionExt {
    fn project(&self, zoom: u8) -> Pixels;

    /// Tile this position is on.
    fn tile_id(&self, zoom: u8) -> TileId;
}

/// Size of the tiles used by the services like the OSM.
const TILE_SIZE: u32 = 256;

fn mercator_normalized((x, y): (f64, f64)) -> (f64, f64) {
    // Project into Mercator (cylindrical map projection).
    let x = x.to_radians();
    let y = y.to_radians().tan().asinh();

    // Scale both x and y to 0-1 range.
    let x = (1. + (x / PI)) / 2.;
    let y = (1. - (y / PI)) / 2.;

    (x, y)
}

impl PositionExt for Position {
    fn project(&self, zoom: u8) -> Pixels {
        let (x, y) = mercator_normalized((*self).into());

        // Map that into a big bitmap made out of web tiles.
        let number_of_pixels = 2u32.pow(zoom as u32) * TILE_SIZE;
        let x = x * number_of_pixels as f64;
        let y = y * number_of_pixels as f64;

        Pixels::new(x as f32, y as f32)
    }

    fn tile_id(&self, zoom: u8) -> TileId {
        let (x, y) = mercator_normalized((*self).into());

        // Map that into a big bitmap made out of web tiles.
        let number_of_tiles = 2u32.pow(zoom as u32);
        let x = (x * number_of_tiles as f64).floor() as u32;
        let y = (y * number_of_tiles as f64).floor() as u32;

        TileId { x, y, zoom }
    }
}

/// Coordinates of the OSM-like tile.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct TileId {
    /// X number of the tile.
    pub x: u32,

    /// Y number of the tile.
    pub y: u32,

    /// Zoom level, where 0 means no zoom.
    /// See: https://wiki.openstreetmap.org/wiki/Zoom_levels
    pub zoom: u8,
}

impl TileId {
    /// Tile position (in pixels) on the "World bitmap".
    pub fn project(&self) -> Pixels {
        Pixels::new((self.x * TILE_SIZE) as f32, (self.y * TILE_SIZE) as f32)
    }

    pub fn east(&self) -> TileId {
        TileId {
            x: self.x + 1,
            y: self.y,
            zoom: self.zoom,
        }
    }

    pub fn west(&self) -> TileId {
        TileId {
            x: self.x - 1,
            y: self.y,
            zoom: self.zoom,
        }
    }

    pub fn north(&self) -> TileId {
        TileId {
            x: self.x,
            y: self.y - 1,
            zoom: self.zoom,
        }
    }

    pub fn south(&self) -> TileId {
        TileId {
            x: self.x,
            y: self.y + 1,
            zoom: self.zoom,
        }
    }
}

/// Transforms screen pixels into position.
///
/// Particularly useful for determining the amount by which the geographical position should be
/// shifted when the screen is dragged.
pub fn screen_to_position(pixels: Vec2, zoom: u8) -> Position {
    let number_of_tiles = 2u32.pow(zoom as u32) as f64 * TILE_SIZE as f64;
    let lat = 1. - (2. * pixels.y as f64 / number_of_tiles);
    let lat = PI * lat - PI;
    let lat = lat.sinh().atan().to_degrees();
    let lon = pixels.x as f64 / number_of_tiles * 360.;
    Position::new(lon, lat)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projecting_position_and_tile() {
        let citadel = Position::new(21.00027, 52.26470);

        let zoom = 16;

        assert_eq!(
            TileId {
                x: 36590,
                y: 21569,
                zoom
            },
            citadel.tile_id(zoom)
        );

        // Projected tile is just its x, y multiplied by the size of tiles.
        assert_eq!(
            Pos2::new(36590. * 256., 21569. * 256.),
            citadel.tile_id(zoom).project()
        );

        // Projected Citadel position should be somewhere near projected tile, shifted only by the
        // position on the tile.
        assert_eq!(
            Pixels::new(36590. * 256. + 252., 21569. * 256. + 7.5),
            citadel.project(zoom)
        );
    }

    #[test]
    fn screen_to_position_works() {
        assert_eq!(
            Position::new(2.1457672119140625e-5, -4.2915344236616925e-5),
            screen_to_position(Vec2::new(1., 2.), 16)
        );
    }
}
