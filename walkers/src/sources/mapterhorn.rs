use super::{Attribution, TileUrls};
use crate::tiles::TileId;

/// Elevation of the whole world, as Terrarium-encoded WebP. Draw it with a `hillshade` style layer,
/// from an [`crate::HttpSource`] made [`with_elevation`](crate::HttpSource::with_elevation) of
/// [`crate::DemEncoding::Terrarium`].
///
/// <https://mapterhorn.com/>
pub struct Mapterhorn;

impl TileUrls for Mapterhorn {
    fn tile_url(&self, tile_id: TileId) -> String {
        format!(
            "https://tiles.mapterhorn.com/{}/{}/{}.webp",
            tile_id.zoom, tile_id.x, tile_id.y
        )
    }

    fn attribution(&self) -> Attribution {
        Attribution {
            text: "Mapterhorn",
            url: "https://mapterhorn.com/attribution",
            logo_light: None,
            logo_dark: None,
        }
    }

    fn tile_size(&self) -> u32 {
        512
    }

    /// Zoom levels above it cover only some parts of the world.
    fn max_zoom(&self) -> u8 {
        12
    }
}
