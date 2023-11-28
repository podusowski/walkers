//! Some common tile map providers.

use std::hash::Hash;

use crate::mercator::TileId;

#[derive(Clone)]
pub struct Attribution {
    pub text: &'static str,
    pub url: &'static str,
    pub logo_light: Option<egui::ImageSource<'static>>,
    pub logo_dark: Option<egui::ImageSource<'static>>,
}

pub trait TileSource {
    fn tile_url(&self, tile_id: TileId) -> String;
    fn attribution(&self) -> Attribution;

    /// Size of each tile, should be a multiple of 256
    fn tile_size(&self) -> u32 {
        256
    }
}

/// <https://www.openstreetmap.org/about>
#[derive(Hash)]
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
            text: "OpenStreetMap contributors",
            url: "https://www.openstreetmap.org/copyright",
            logo_light: None,
            logo_dark: None,
        }
    }
}

/// Orthophotomap layer from Poland's Geoportal.
/// <https://www.geoportal.gov.pl/uslugi/usluga-przegladania-wms>
#[derive(Hash)]
pub struct Geoportal;

impl TileSource for Geoportal {
    fn tile_url(&self, tile_id: TileId) -> String {
        format!(
            "https://mapy.geoportal.gov.pl/wss/service/PZGIK/ORTO/WMTS/StandardResolution?\
            &SERVICE=WMTS\
            &REQUEST=GetTile\
            &VERSION=1.0.0\
            &LAYER=ORTOFOTOMAPA\
            &TILEMATRIXSET=EPSG:3857\
            &TILEMATRIX=EPSG:3857:{}\
            &TILEROW={}\
            &TILECOL={}",
            tile_id.zoom, tile_id.y, tile_id.x
        )
    }

    fn attribution(&self) -> Attribution {
        Attribution {
            text: "Główny Urząd Geodezji i Kartografii",
            url: "https://www.geoportal.gov.pl/",
            logo_light: None,
            logo_dark: None,
        }
    }
}

/// Predefined Mapbox styles.
/// <https://docs.mapbox.com/api/maps/styles/#classic-mapbox-styles>
#[derive(Clone, Copy, Default, Hash)]
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
#[derive(Default, Hash)]
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
            logo_light: Some(egui::include_image!("../assets/mapbox-logo-white.svg")),
            logo_dark: Some(egui::include_image!("../assets/mapbox-logo-black.svg")),
        }
    }

    fn tile_size(&self) -> u32 {
        512
    }
}
