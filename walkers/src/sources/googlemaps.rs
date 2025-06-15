use crate::TileId;

use super::{Attribution, TileSource};

#[derive(Clone, Copy, Default)]
pub enum GoogleMapsStyle {
    #[default]
    /// Default Google Maps style, which is a streets view
    Default,
    /// Satellite view
    Satellite,
    /// Hybrid view
    Hybrid,
}

impl GoogleMapsStyle {
    fn api_slug(&self) -> &'static str {
        match self {
            Self::Default => "m",
            Self::Satellite => "s",
            Self::Hybrid => "y",
        }
    }
}

#[derive(Default)]
pub struct GoogleMaps {
    /// Style of the map, default is `GoogleMapsStyle::Default`
    pub style: GoogleMapsStyle,
}

impl TileSource for GoogleMaps {
    fn tile_url(&self, tile_id: TileId) -> String {
        format!(
            "https://mt1.google.com/vt/lyrs={}&x={}&y={}&z={}",
            self.style.api_slug(),
            tile_id.x,
            tile_id.y,
            tile_id.zoom
        )
    }

    fn attribution(&self) -> Attribution {
        Attribution {
            text: "Map data © Google",
            url: "https://www.google.com/intl/en_us/help/terms_maps/",
            logo_light: None,
            logo_dark: None,
        }
    }

    fn max_zoom(&self) -> u8 {
        21 // Google Maps supports up to zoom level 21
    }
}
