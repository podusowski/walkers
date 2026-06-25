use super::{Attribution, TileSource};
use crate::tiles::TileId;

#[derive(Debug, Clone, Copy)]
pub enum OpenTopoServer {
    A,
    B,
    C,
}

impl std::fmt::Display for OpenTopoServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpenTopoServer::A => write!(f, "a"),
            OpenTopoServer::B => write!(f, "b"),
            OpenTopoServer::C => write!(f, "c"),
        }
    }
}

/// <https://www.opentopomap.org/about>
pub struct OpenTopoMap(pub OpenTopoServer);

impl TileSource for OpenTopoMap {
    fn tile_url(&self, tile_id: TileId) -> String {
        format!(
            "https://{}.tile.opentopomap.org/{}/{}/{}.png",
            self.0, tile_id.zoom, tile_id.x, tile_id.y
        )
    }

    fn attribution(&self) -> Attribution {
        Attribution {
            text: "Map data: © OpenStreetMap contributors, SRTM | Map presentation: © OpenTopoMap (CC-BY-SA)",
            url: "https://www.opentopomap.org/about",
            logo_light: None,
            logo_dark: None,
        }
    }
}
