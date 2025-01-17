use super::{Attribution, TileSource};
use crate::tiles::TileId;

/// <https://www.openstreetmap.org/about>
pub struct OpenStreetMap;

impl TileSource for OpenStreetMap {
    fn tile_url(&self, tile_id: TileId) -> String {
        format!(
            "https://tile.openstreetmap.org/{}/{}/{}.png",
            tile_id.zoom, tile_id.x, tile_id.y
        )
    }

    fn attribution(&self) -> Attribution {
        Attribution {
            text: "© OpenStreetMap contributors",
            url: "https://www.openstreetmap.org/copyright",
            logo_light: Some(egui::include_image!("../../assets/mapbox-logo-white.svg")),
            logo_dark: Some(egui::include_image!("../../assets/mapbox-logo-black.svg")),
        }
    }
}
