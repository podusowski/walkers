use super::{Attribution, TileSource};
use crate::TileId;

/// <https://www.openstreetmap.org/about>
pub struct OpenFreeMap;

impl TileSource for OpenFreeMap {
    fn tile_url(&self, tile_id: TileId) -> String {
        format!(
            "https://tiles.openfreemap.org/planet/20251203_001001_pt/{}/{}/{}.pbf",
            tile_id.zoom, tile_id.x, tile_id.y
        )
    }

    fn attribution(&self) -> Attribution {
        Attribution {
            text: "OpenFreeMap OpenStreetMap contributors",
            url: "https://www.openstreetmap.org/copyright",
            logo_light: None,
            logo_dark: None,
        }
    }

    fn tile_size(&self) -> u32 {
        1024
    }

    fn max_zoom(&self) -> u8 {
        16
    }
}
