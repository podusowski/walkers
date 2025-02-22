//! Project the lat/lon coordinates into a 2D x/y using the Web Mercator.
//! <https://en.wikipedia.org/wiki/Web_Mercator_projection>
//! <https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames>
//! <https://www.netzwolf.info/osm/tilebrowser.html?lat=51.157800&lon=6.865500&zoom=14>

use crate::{
    lon_lat,
    position::{Pixels, Position},
};
use std::f64::consts::PI;

// zoom level   tile coverage  number of tiles  tile size(*) in degrees
// 0            1 tile         1 tile           360° x 170.1022°
// 1            2 × 2 tiles    4 tiles          180° x 85.0511°
// 2            4 × 4 tiles    16 tiles         90° x [variable]

/// Zoom specifies how many pixels are in the whole map. For example, zoom 0 means that the whole
/// map is just one 256x256 tile, zoom 1 means that it is 2x2 tiles, and so on.
pub(crate) fn total_pixels(zoom: f64) -> f64 {
    2f64.powf(zoom) * (TILE_SIZE as f64)
}

pub fn total_tiles(zoom: u8) -> u32 {
    2u32.pow(zoom as u32)
}

/// Size of a single tile in pixels. Walkers uses 256px tiles as most of the tile sources do.
const TILE_SIZE: u32 = 256;

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

/// Coordinates of the OSM-like tile.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct TileId {
    /// X number of the tile.
    pub x: u32,

    /// Y number of the tile.
    pub y: u32,

    /// Zoom level, where 0 means no zoom.
    /// See: <https://wiki.openstreetmap.org/wiki/Zoom_levels>
    pub zoom: u8,
}

impl TileId {
    /// Tile position (in pixels) on the "World bitmap".
    pub fn project(&self, tile_size: f64) -> Pixels {
        Pixels::new(self.x as f64 * tile_size, self.y as f64 * tile_size)
    }

    pub fn east(&self) -> Option<TileId> {
        (self.x < total_tiles(self.zoom) - 1).then_some(TileId {
            x: self.x + 1,
            y: self.y,
            zoom: self.zoom,
        })
    }

    pub fn west(&self) -> Option<TileId> {
        Some(TileId {
            x: self.x.checked_sub(1)?,
            y: self.y,
            zoom: self.zoom,
        })
    }

    pub fn north(&self) -> Option<TileId> {
        Some(TileId {
            x: self.x,
            y: self.y.checked_sub(1)?,
            zoom: self.zoom,
        })
    }

    pub fn south(&self) -> Option<TileId> {
        (self.y < total_tiles(self.zoom) - 1).then_some(TileId {
            x: self.x,
            y: self.y + 1,
            zoom: self.zoom,
        })
    }
}

/// Calculate the tile coordinated for the given position.
pub(crate) fn tile_id(position: Position, mut zoom: u8, source_tile_size: u32) -> TileId {
    let (x, y) = mercator_normalized(position);

    // Some sources provide larger tiles, effectively bundling e.g. 4 256px tiles in one
    // 512px one. Walkers uses 256px internally, so we need to adjust the zoom level.
    zoom -= (source_tile_size as f64 / TILE_SIZE as f64).log2() as u8;

    // Map that into a big bitmap made out of web tiles.
    let number_of_tiles = 2u32.pow(zoom as u32) as f64;
    let x = (x * number_of_tiles).floor() as u32;
    let y = (y * number_of_tiles).floor() as u32;

    TileId { x, y, zoom }
}

/// Project geographical position into a 2D plane using Mercator.
pub(crate) fn project(position: Position, zoom: f64) -> Pixels {
    let total_pixels = total_pixels(zoom);
    let (x, y) = mercator_normalized(position);
    Pixels::new(x * total_pixels, y * total_pixels)
}

/// Transforms screen pixels into a geographical position.
pub fn screen_to_position(pixels: Pixels, zoom: f64) -> Position {
    let number_of_pixels: f64 = 2f64.powf(zoom) * (TILE_SIZE as f64);

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

    #[test]
    fn projecting_position_and_tile() {
        let citadel = lon_lat(21.00027, 52.26470);

        // Just a bit higher than what most providers support,
        // to make sure we cover the worst case in terms of precision.
        let zoom = 20;

        assert_eq!(
            TileId {
                x: 585455,
                y: 345104,
                zoom
            },
            tile_id(citadel, zoom, 256)
        );

        // Automatically zooms out for larger tiles
        assert_eq!(
            TileId {
                x: 292727,
                y: 172552,
                zoom: zoom - 1
            },
            tile_id(citadel, zoom, 512)
        );

        // Projected tile is just its x, y multiplied by the size of tiles.
        assert_eq!(
            Pixels::new(585455. * 256., 345104. * 256.),
            tile_id(citadel, zoom, 256).project(256.)
        );

        // Projected Citadel position should be somewhere near projected tile, shifted only by the
        // position on the tile.
        let calculated = project(citadel, zoom as f64);
        let citadel_proj = Pixels::new(585455. * 256. + 184., 345104. * 256. + 116.5);
        approx::assert_relative_eq!(calculated.x(), citadel_proj.x(), max_relative = 0.5);
        approx::assert_relative_eq!(calculated.y(), citadel_proj.y(), max_relative = 0.5);
    }

    #[test]
    fn project_there_and_back() {
        let citadel = lat_lon(21.00027, 52.26470);
        let zoom = 16;
        let calculated = screen_to_position(project(citadel, zoom as f64), zoom as f64);

        approx::assert_relative_eq!(calculated.x(), citadel.x(), max_relative = 1.0);
        approx::assert_relative_eq!(calculated.y(), citadel.y(), max_relative = 1.0);
    }

    #[test]
    fn tile_id_cannot_go_beyond_limits() {
        // There is only one tile at zoom 0.
        let tile_id = TileId {
            x: 0,
            y: 0,
            zoom: 0,
        };

        assert_eq!(tile_id.west(), None);
        assert_eq!(tile_id.north(), None);
        assert_eq!(tile_id.south(), None);
        assert_eq!(tile_id.east(), None);

        // There are 2 tiles at zoom 1.
        let tile_id = TileId {
            x: 0,
            y: 0,
            zoom: 1,
        };

        assert_eq!(tile_id.west(), None);
        assert_eq!(tile_id.north(), None);

        assert_eq!(
            tile_id.south(),
            Some(TileId {
                x: 0,
                y: 1,
                zoom: 1
            })
        );

        assert_eq!(
            tile_id.east(),
            Some(TileId {
                x: 1,
                y: 0,
                zoom: 1
            })
        );
    }
}
