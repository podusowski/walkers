use std::{collections::BTreeMap, path::PathBuf};

use egui::Context;
use walkers::{HttpOptions, HttpTiles, LocalTiles, Tiles};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Provider {
    OpenStreetMap,
    Geoportal,
    OpenStreetMapWithGeoportal,
    MapboxStreets,
    MapboxSatellite,
    LocalTiles,
    YandexMaps,
}

pub(crate) enum TilesKind {
    Http(HttpTiles),
    Local(LocalTiles),
}

impl AsMut<dyn Tiles> for TilesKind {
    fn as_mut(&mut self) -> &mut (dyn Tiles + 'static) {
        match self {
            TilesKind::Http(tiles) => tiles,
            TilesKind::Local(tiles) => tiles,
        }
    }
}

impl AsRef<dyn Tiles> for TilesKind {
    fn as_ref(&self) -> &(dyn Tiles + 'static) {
        match self {
            TilesKind::Http(tiles) => tiles,
            TilesKind::Local(tiles) => tiles,
        }
    }
}

fn http_options() -> HttpOptions {
    HttpOptions {
        // Not sure where to put cache on Android, so it will be disabled for now.
        cache: if cfg!(target_os = "android") || std::env::var("NO_HTTP_CACHE").is_ok() {
            None
        } else {
            Some(".cache".into())
        },
        ..Default::default()
    }
}

pub(crate) fn providers(egui_ctx: Context) -> BTreeMap<Provider, Vec<TilesKind>> {
    let mut providers = BTreeMap::default();

    providers.insert(
        Provider::OpenStreetMap,
        vec![TilesKind::Http(HttpTiles::with_options(
            walkers::sources::OpenStreetMap,
            http_options(),
            egui_ctx.to_owned(),
        ))],
    );

    providers.insert(
        Provider::Geoportal,
        vec![TilesKind::Http(HttpTiles::with_options(
            walkers::sources::Geoportal,
            http_options(),
            egui_ctx.to_owned(),
        ))],
    );

    providers.insert(
        Provider::OpenStreetMapWithGeoportal,
        vec![
            TilesKind::Http(HttpTiles::with_options(
                walkers::sources::OpenStreetMap,
                http_options(),
                egui_ctx.to_owned(),
            )),
            TilesKind::Http(HttpTiles::with_options(
                walkers::sources::Geoportal,
                http_options(),
                egui_ctx.to_owned(),
            )),
        ],
    );

    providers.insert(
        Provider::Geoportal,
        vec![TilesKind::Http(HttpTiles::with_options(
            walkers::sources::Geoportal,
            http_options(),
            egui_ctx.to_owned(),
        ))],
    );

    providers.insert(
        Provider::LocalTiles,
        vec![TilesKind::Local(LocalTiles::new(
            PathBuf::from_iter(&[env!("CARGO_MANIFEST_DIR"), "assets"]),
            egui_ctx.to_owned(),
        ))],
    );

    // Pass in a mapbox access token at compile time. May or may not be what you want to do,
    // potentially loading it from application settings instead.
    let mapbox_access_token = std::option_env!("MAPBOX_ACCESS_TOKEN");

    // We only show the mapbox map if we have an access token
    if let Some(token) = mapbox_access_token {
        providers.insert(
            Provider::MapboxStreets,
            vec![TilesKind::Http(HttpTiles::with_options(
                walkers::sources::Mapbox {
                    style: walkers::sources::MapboxStyle::Streets,
                    access_token: token.to_string(),
                    high_resolution: false,
                },
                http_options(),
                egui_ctx.to_owned(),
            ))],
        );
        providers.insert(
            Provider::MapboxSatellite,
            vec![TilesKind::Http(HttpTiles::with_options(
                walkers::sources::Mapbox {
                    style: walkers::sources::MapboxStyle::Satellite,
                    access_token: token.to_string(),
                    high_resolution: true,
                },
                http_options(),
                egui_ctx.to_owned(),
            ))],
        );
    }

    let yandex_access_token = std::option_env!("YANDEX_ACCESS_TOKEN");

    if let Some(token) = yandex_access_token {
        providers.insert(
            Provider::YandexMaps,
            vec![TilesKind::Http(HttpTiles::with_options(
                walkers::sources::YandexMaps {
                    language: walkers::sources::YandexMapsLanguage::EnUS,
                    projection: walkers::sources::YandexMapsProjection::WebMercator,
                    maptype: walkers::sources::YandexMapsMapType::FutureMap,
                    access_token: token.to_string(),
                },
                http_options(),
                egui_ctx.to_owned(),
            ))],
        );
    }

    providers
}
