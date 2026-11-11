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

    /// Only some places, the Alps for example. Most of Europe, North America and Japan go up to
    /// 16, and the rest of the world to 12. Where a tile is missing, a lower zoom level's is
    /// drawn instead.
    fn max_zoom(&self) -> u8 {
        17
    }
}
