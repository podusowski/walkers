//! Some common tile map providers.

use crate::mercator::TileId;

/// <https://www.openstreetmap.org/about>
pub fn openstreetmap(tile_id: TileId) -> String {
    format!(
        "https://tile.openstreetmap.org/{}/{}/{}.png",
        tile_id.zoom, tile_id.x, tile_id.y
    )
}
