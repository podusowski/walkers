use super::{Attribution, TileSource};
use crate::TileId;

pub struct OpenFreeMap;

impl TileSource for OpenFreeMap {
    fn tile_url(&self, tile_id: TileId) -> String {
        format!(
            "https://tiles.openfreemap.org/planet/20251217_001001_pt/{}/{}/{}.pbf",
            tile_id.zoom, tile_id.x, tile_id.y
        )
    }

    fn attribution(&self) -> Attribution {
        Attribution {
            text: "OpenFreeMap © OpenMapTiles Data from OpenStreetMap",
            url: "https://openfreemap.org",
            logo_light: None,
            logo_dark: None,
        }
    }

    fn tile_size(&self) -> u32 {
        512
    }
}
