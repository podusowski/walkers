use crate::TileId;

use super::{Attribution, TileSource};

/// Predefined Mapbox styles.
/// <https://docs.mapbox.com/api/maps/styles/#classic-mapbox-styles>
#[derive(Clone, Copy, Default)]
pub enum MapboxStyle {
    #[default]
    Streets,
    Outdoors,
    Light,
    Dark,
    Satellite,
    SatelliteStreets,
    NavigationDay,
    NavigationNight,
}

impl MapboxStyle {
    fn api_slug(&self) -> &'static str {
        match self {
            Self::Streets => "streets-v12",
            Self::Outdoors => "outdoors-v12",
            Self::Light => "light-v11",
            Self::Dark => "dark-v11",
            Self::Satellite => "satellite-v9",
            Self::SatelliteStreets => "satellite-streets-v12",
            Self::NavigationDay => "navigation-day-v1",
            Self::NavigationNight => "navigation-night-v1",
        }
    }
}

/// Mapbox static tile source.
/// <https://docs.mapbox.com/api/maps/static-tiles/>
#[derive(Default)]
pub struct Mapbox {
    /// Predefined style to use
    pub style: MapboxStyle,
    /// Render tiles at 1024x1024 instead of 512x512 (@2x)
    pub high_resolution: bool,
    /// Mapbox API key, required
    pub access_token: String,
}

impl TileSource for Mapbox {
    fn tile_url(&self, tile_id: TileId) -> String {
        format!(
            "https://api.mapbox.com/styles/v1/mapbox/{}/tiles/512/{}/{}/{}{}?access_token={}",
            self.style.api_slug(),
            tile_id.zoom,
            tile_id.x,
            tile_id.y,
            if self.high_resolution { "@2x" } else { "" },
            self.access_token
        )
    }

    fn attribution(&self) -> Attribution {
        // TODO: Proper linking (https://docs.mapbox.com/help/getting-started/attribution/))
        Attribution {
            text: "© Mapbox, © OpenStreetMap",
            url: "https://www.mapbox.com/about/maps/",
            logo_light: Some(egui::include_image!("../../assets/mapbox-logo-white.svg")),
            logo_dark: Some(egui::include_image!("../../assets/mapbox-logo-black.svg")),
        }
    }

    fn tile_size(&self) -> u32 {
        512
    }
}
