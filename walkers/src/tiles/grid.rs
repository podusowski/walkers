//! The grid of tiles laid over the projected map, whatever the projection.
//! <https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames>
//! <https://www.netzwolf.info/osm/tilebrowser.html?lat=51.157800&lon=6.865500&zoom=14>

use super::TileId;
use crate::position::Pixels;

/// Zoom specifies how many pixels are in the whole map. For example, zoom 0 means that the whole
/// map is just one 256x256 tile, zoom 1 means that it is 2x2 tiles, and so on.
pub(crate) fn total_pixels(zoom: f64) -> f64 {
    2f64.powf(zoom) * (TILE_SIZE as f64)
}

pub(crate) fn total_tiles(zoom: u8) -> u32 {
    2u32.pow(zoom as u32)
}

/// Size of a single tile in pixels. Walkers uses 256px tiles as most of the tile sources do.
pub(crate) const TILE_SIZE: u32 = 256;

/// Highest zoom level a [`TileId`] can have. Above this, the number of tiles no longer fits
/// in the `u32` used for the tile coordinates.
const MAX_TILE_ZOOM: u8 = 31;

/// How many zoom levels a source with `source_tile_size` tiles is off from the 256px grid
/// Walkers uses internally. Positive for larger tiles, negative for smaller ones.
pub(crate) fn zoom_offset(source_tile_size: u32) -> i32 {
    (source_tile_size as f64 / TILE_SIZE as f64).log2().round() as i32
}

/// Calculate the tile coordinates for the given pixels of the map at `zoom`.
pub(crate) fn tile_id_at(pixels: Pixels, zoom: f64, source_tile_size: u32) -> TileId {
    let total_pixels = total_pixels(zoom);
    tile_id_normalized(
        pixels.x() / total_pixels,
        pixels.y() / total_pixels,
        zoom.round() as u8,
        source_tile_size,
    )
}

/// Calculate the tile coordinates for the given position normalized to 0-1 range.
fn tile_id_normalized(x: f64, y: f64, zoom: u8, source_tile_size: u32) -> TileId {
    // Some sources provide tiles of a different size, effectively bundling e.g. 4 256px tiles
    // in one 512px one, or splitting a single one into 4 128px ones. Walkers uses 256px
    // internally, so we need to adjust the zoom level in either direction.
    let zoom = (zoom as i32).saturating_sub(zoom_offset(source_tile_size));

    // Clamp, since there is nothing below the single tile covering the whole world, and going
    // too far up would overflow the number of tiles.
    let zoom = zoom.clamp(0, MAX_TILE_ZOOM as i32) as u8;

    // Map that into a big bitmap made out of web tiles.
    let number_of_tiles = 2u32.pow(zoom as u32) as f64;
    let x = (x * number_of_tiles).floor() as u32;
    let y = (y * number_of_tiles).floor() as u32;

    TileId { x, y, zoom }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MercatorProjection, Position, Projection, lon_lat};

    fn project(position: Position, zoom: f64) -> Pixels {
        MercatorProjection.project(position, zoom)
    }

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
            tile_id_at(project(citadel, zoom as f64), zoom as f64, 256)
        );

        // Automatically zooms out for larger tiles
        assert_eq!(
            TileId {
                x: 292727,
                y: 172552,
                zoom: zoom - 1
            },
            tile_id_at(project(citadel, zoom as f64), zoom as f64, 512)
        );

        // Projected tile is just its x, y multiplied by the size of tiles.
        assert_eq!(
            Pixels::new(585455. * 256., 345104. * 256.),
            tile_id_at(project(citadel, zoom as f64), zoom as f64, 256).project(256.)
        );

        // Projected Citadel position should be somewhere near projected tile, shifted only by the
        // position on the tile.
        let calculated = project(citadel, zoom as f64);
        let citadel_proj = Pixels::new(585455. * 256. + 184., 345104. * 256. + 116.5);
        approx::assert_relative_eq!(calculated.x(), citadel_proj.x(), max_relative = 0.5);
        approx::assert_relative_eq!(calculated.y(), citadel_proj.y(), max_relative = 0.5);
    }

    /// Tiles smaller than 256px need *more* tiles to cover the same area, which means going
    /// one zoom level deeper for every halving of the tile size.
    /// See: <https://github.com/podusowski/walkers/issues/551>
    #[test]
    fn smaller_tiles_zoom_in() {
        let citadel = lon_lat(21.00027, 52.26470);
        let zoom = 20;

        // The citadel is at 184x116 of its 256px tile, so it falls into the right-top
        // quadrant once that tile is split into four 128px ones.
        assert_eq!(
            TileId {
                x: 585455 * 2 + 1,
                y: 345104 * 2,
                zoom: zoom + 1
            },
            tile_id_at(project(citadel, zoom as f64), zoom as f64, 128)
        );

        // Whatever the tile size, the smaller tile has to lie within the 256px one covering
        // the same position.
        for (tile_size, levels) in [(128, 1), (64, 2), (32, 3)] {
            let base = tile_id_at(project(citadel, zoom as f64), zoom as f64, 256);
            let smaller = tile_id_at(project(citadel, zoom as f64), zoom as f64, tile_size);
            let ratio = 2u32.pow(levels);

            assert_eq!(base.zoom + levels as u8, smaller.zoom);
            assert_eq!(base.x, smaller.x / ratio);
            assert_eq!(base.y, smaller.y / ratio);
        }
    }

    /// Number of tiles is kept in `u32`, so the zoom level cannot grow indefinitely, no matter
    /// how small the tiles of a source are.
    #[test]
    fn zoomed_in_further_than_tile_coordinates_allow() {
        let citadel = lon_lat(21.00027, 52.26470);

        assert_eq!(
            MAX_TILE_ZOOM,
            tile_id_at(project(citadel, 26.), 26., 1).zoom
        );

        // Degenerate, but it should not panic either.
        assert_eq!(
            MAX_TILE_ZOOM,
            tile_id_at(project(citadel, 26.), 26., 0).zoom
        );
    }

    #[test]
    fn zoomed_out_further_than_source_tile_size_allows() {
        let citadel = lon_lat(21.00027, 52.26470);

        // Large tiles mean lower zoom levels, but there is nothing below the single tile
        // covering the whole world.
        assert_eq!(
            TileId {
                x: 0,
                y: 0,
                zoom: 0
            },
            tile_id_at(project(citadel, 0.), 0., 512)
        );
        assert_eq!(
            TileId {
                x: 0,
                y: 0,
                zoom: 0
            },
            tile_id_at(project(citadel, 1.), 1., 1024)
        );
    }
}
