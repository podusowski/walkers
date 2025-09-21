use std::{collections::BTreeMap, path::PathBuf};

use egui::Context;
use walkers::{HttpOptions, HttpTiles, LocalTiles, PmTiles, Tiles};

pub(crate) enum TilesKind {
    Http(HttpTiles),
    Local(LocalTiles),
    PmTiles(PmTiles),
}

impl AsMut<dyn Tiles> for TilesKind {
    fn as_mut(&mut self) -> &mut (dyn Tiles + 'static) {
        match self {
            TilesKind::Http(tiles) => tiles,
            TilesKind::Local(tiles) => tiles,
            TilesKind::PmTiles(tiles) => tiles,
        }
    }
}

impl AsRef<dyn Tiles> for TilesKind {
    fn as_ref(&self) -> &(dyn Tiles + 'static) {
        match self {
            TilesKind::Http(tiles) => tiles,
            TilesKind::Local(tiles) => tiles,
            TilesKind::PmTiles(tiles) => tiles,
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

pub(crate) fn providers(egui_ctx: Context) -> BTreeMap<String, Vec<TilesKind>> {
    let mut providers = BTreeMap::default();

    providers.insert(
        "OpenStreetMap".to_string(),
        vec![TilesKind::Http(HttpTiles::with_options(
            walkers::sources::OpenStreetMap,
            http_options(),
            egui_ctx.to_owned(),
        ))],
    );

    providers.insert(
        "Geoportal".to_string(),
        vec![TilesKind::Http(HttpTiles::with_options(
            walkers::sources::Geoportal,
            http_options(),
            egui_ctx.to_owned(),
        ))],
    );

    providers.insert(
        "OpenStreetMapWithGeoportal".to_string(),
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
        "Geoportal".to_string(),
        vec![TilesKind::Http(HttpTiles::with_options(
            walkers::sources::Geoportal,
            http_options(),
            egui_ctx.to_owned(),
        ))],
    );

    providers.insert(
        "LocalTiles".to_string(),
        vec![TilesKind::Local(LocalTiles::new(
            PathBuf::from_iter(&[env!("CARGO_MANIFEST_DIR"), "assets"]),
            egui_ctx.to_owned(),
        ))],
    );

    providers.insert(
        "LocalPmTiles".to_string(),
        vec![TilesKind::PmTiles(PmTiles::new(PathBuf::from_iter(&[
            env!("CARGO_MANIFEST_DIR"),
            "wroclaw.pmtiles",
        ])))],
    );

    providers.insert(
        "LocalPmTilesPlanet".to_string(),
        vec![TilesKind::PmTiles(PmTiles::new(PathBuf::from_iter(&[
            env!("CARGO_MANIFEST_DIR"),
            "planet_z6.pmtiles",
        ])))],
    );

    // Pass in a mapbox access token at compile time. May or may not be what you want to do,
    // potentially loading it from application settings instead.
    let mapbox_access_token = std::option_env!("MAPBOX_ACCESS_TOKEN");

    // We only show the mapbox map if we have an access token
    if let Some(token) = mapbox_access_token {
        providers.insert(
            "MapboxStreets".to_string(),
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
            "MapboxSatellite".to_string(),
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

    providers
}

fn find_pmtiles_files() -> Vec<PathBuf> {
    let Ok(dir) = std::fs::read_dir(".") else {
        return Vec::new();
    };

    dir.filter_map(|entry| {
        let path = entry.ok()?.path();
        if path.extension()?.to_str()? == "pmtiles" {
            Some(path)
        } else {
            None
        }
    })
    .collect::<Vec<_>>()
}
