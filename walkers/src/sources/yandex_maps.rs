use crate::TileId;

use super::{Attribution, TileSource};

/// Yandex Maps projection type
/// <https://yandex.com/maps-api/docs/tiles-api/index.html>
#[derive(Clone, Copy, Default)]
pub enum YandexMapsProjection {
    /// Spherical Mercator projection (Web Mercator)
    #[default]
    WebMercator,
    /// Elliptical Mercator projection (WGS 84)
    Wgs84Mercator,
}

impl YandexMapsProjection {
    fn api_slug(&self) -> &'static str {
        match self {
            Self::Wgs84Mercator => "wgs84_mercator",
            Self::WebMercator => "web_mercator",
        }
    }
}

/// Yandex Maps map type
/// <https://yandex.com/maps-api/docs/tiles-api/index.html>
#[derive(Clone, Copy, Default)]
pub enum YandexMapsMapType {
    /// Basic map (default)
    #[default]
    Map,
    /// Basic map with updated design
    FutureMap,
    /// Map for car navigation
    Driving,
    /// Public transport map
    Transit,
    /// Administrative map
    Admin,
}

impl YandexMapsMapType {
    fn api_slug(&self) -> &'static str {
        match self {
            Self::Map => "map",
            Self::FutureMap => "future_map",
            Self::Driving => "driving",
            Self::Transit => "transit",
            Self::Admin => "admin",
        }
    }
}

/// Language and region settings for map labels
/// Format: language_region, where:
/// - language: two-letter language code (ISO 639-1)
/// - region: two-letter country code (ISO 3166-1)
#[derive(Clone, Copy, Default)]
pub enum YandexMapsLanguage {
    /// Russian labels, Russian region settings
    #[default]
    RuRU,
    /// English labels, Russian region settings
    EnRU,
    /// English labels, US region settings
    EnUS,
    /// Ukrainian labels, Ukrainian region settings
    UkUA,
    /// Russian labels, Ukrainian region settings
    RuUA,
    /// Turkish labels, Turkish region settings
    TrTR,
}

impl YandexMapsLanguage {
    fn api_slug(&self) -> &'static str {
        match self {
            Self::RuRU => "ru_RU",
            Self::EnRU => "en_RU",
            Self::EnUS => "en_US",
            Self::UkUA => "uk_UA",
            Self::RuUA => "ru_UA",
            Self::TrTR => "tr_TR",
        }
    }
}

/// Yandex Maps Tiles API
/// <https://yandex.com/maps-api/docs/tiles-api/index.html>
#[derive(Default)]
pub struct YandexMaps {
    /// Projection type to use
    pub projection: YandexMapsProjection,
    /// Map type to use
    pub maptype: YandexMapsMapType,
    /// Language and region settings
    pub language: YandexMapsLanguage,
    /// Yandex Maps Tiles API key, required
    pub access_token: String,
}

impl TileSource for YandexMaps {
    fn tile_url(&self, tile_id: TileId) -> String {
        format!(
            "https://tiles.api-maps.yandex.ru/v1/tiles/?apikey={}&lang={}&x={}&y={}&z={}&l=map&scale=2&projection={}&maptype={}",
            self.access_token,
            self.language.api_slug(),
            tile_id.x,
            tile_id.y,
            tile_id.zoom,
            self.projection.api_slug(),
            self.maptype.api_slug(),
        )
    }

    fn attribution(&self) -> Attribution {
        Attribution {
            text: "© Yandex Maps",
            url: "https://yandex.com/maps",
            logo_light: Some(egui::include_image!("../../assets/yandex_logo_en.svg")),
            logo_dark: Some(egui::include_image!("../../assets/yandex_logo_en.svg")),
        }
    }

    fn tile_size(&self) -> u32 {
        512
    }

    fn max_zoom(&self) -> u8 {
        20
    }
}
