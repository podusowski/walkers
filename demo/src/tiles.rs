use std::{collections::BTreeMap, path::PathBuf};

use egui::Context;
#[cfg(feature = "pmtiles")]
use walkers::PmTiles;
use walkers::Style;
use walkers::{HttpOptions, HttpTiles, LocalTiles, Tiles};

pub(crate) enum TilesKind {
    Http(HttpTiles),
    Local(LocalTiles),
    #[cfg(feature = "pmtiles")]
    PmTiles(PmTiles),
}

impl AsMut<dyn Tiles> for TilesKind {
    fn as_mut(&mut self) -> &mut (dyn Tiles + 'static) {
        match self {
            TilesKind::Http(tiles) => tiles,
            TilesKind::Local(tiles) => tiles,
            #[cfg(feature = "pmtiles")]
            TilesKind::PmTiles(tiles) => tiles,
        }
    }
}

impl AsRef<dyn Tiles> for TilesKind {
    fn as_ref(&self) -> &(dyn Tiles + 'static) {
        match self {
            TilesKind::Http(tiles) => tiles,
            TilesKind::Local(tiles) => tiles,
            #[cfg(feature = "pmtiles")]
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

#[derive(Default)]
pub struct Providers {
    pub available: BTreeMap<String, Vec<TilesKind>>,
    pub selected: String,
    #[cfg(feature = "pmtiles")]
    pub have_some_pmtiles: bool,
}

pub(crate) fn providers(egui_ctx: Context) -> Providers {
    let mut providers = Providers::default();

    providers.available.insert(
        "OpenStreetMap".to_string(),
        vec![TilesKind::Http(HttpTiles::with_options(
            walkers::sources::OpenStreetMap,
            http_options(),
            egui_ctx.to_owned(),
        ))],
    );
    providers.selected = "OpenStreetMap".to_string();

    #[cfg(feature = "mvt")]
    providers.available.insert(
        "OpenFreeMap".to_string(),
        vec![TilesKind::Http(HttpTiles::with_options_and_style(
            walkers::sources::OpenFreeMap,
            http_options(),
            Style::openfreemap_bright(),
            egui_ctx.to_owned(),
        ))],
    );

    providers.available.insert(
        "Geoportal".to_string(),
        vec![TilesKind::Http(HttpTiles::with_options(
            walkers::sources::Geoportal,
            http_options(),
            egui_ctx.to_owned(),
        ))],
    );

    providers.available.insert(
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

    providers.available.insert(
        "Geoportal".to_string(),
        vec![TilesKind::Http(HttpTiles::with_options(
            walkers::sources::Geoportal,
            http_options(),
            egui_ctx.to_owned(),
        ))],
    );

    #[allow(deprecated)]
    providers.available.insert(
        "LocalTiles".to_string(),
        vec![TilesKind::Local(LocalTiles::new(
            PathBuf::from_iter(&[env!("CARGO_MANIFEST_DIR"), "assets"]),
            egui_ctx.to_owned(),
        ))],
    );

    #[cfg(feature = "pmtiles")]
    {
        let pmtiles = find_pmtiles_files();
        providers.have_some_pmtiles = !pmtiles.is_empty();

        for path in pmtiles {
            let name = path.file_stem().unwrap().to_string_lossy().to_string();
            providers.available.insert(
                name.clone(),
                vec![TilesKind::PmTiles(PmTiles::with_style(
                    path.clone(),
                    Style::protomaps_dark(),
                    egui_ctx.to_owned(),
                ))],
            );
            providers.selected = name.clone();

            providers.available.insert(
                format!("{} (Protomaps Dark Vis)", name.clone()),
                vec![TilesKind::PmTiles(PmTiles::with_style(
                    path.clone(),
                    Style::protomaps_dark_vis(),
                    egui_ctx.to_owned(),
                ))],
            );

            providers.available.insert(
                format!("{} (Protomaps Light)", name.clone()),
                vec![TilesKind::PmTiles(PmTiles::with_style(
                    path.clone(),
                    Style::protomaps_light(),
                    egui_ctx.to_owned(),
                ))],
            );

            providers.available.insert(
                format!("{name}WithGeoportal"),
                vec![
                    TilesKind::PmTiles(PmTiles::with_style(
                        path,
                        Style::protomaps_dark(),
                        egui_ctx.to_owned(),
                    )),
                    TilesKind::Http(HttpTiles::with_options(
                        walkers::sources::Geoportal,
                        http_options(),
                        egui_ctx.to_owned(),
                    )),
                ],
            );
        }
    }

    // Pass in a mapbox access token at compile time. May or may not be what you want to do,
    // potentially loading it from application settings instead.
    let mapbox_access_token = std::option_env!("MAPBOX_ACCESS_TOKEN");

    // We only show the mapbox map if we have an access token
    if let Some(token) = mapbox_access_token {
        providers.available.insert(
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
        providers.available.insert(
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

#[cfg(feature = "pmtiles")]
fn find_pmtiles_files() -> Vec<PathBuf> {
    let Ok(dir) = std::fs::read_dir(".") else {
        return Vec::new();
    };

    dir.filter_map(|entry| {
        let path = entry.ok()?.path();
        (path.extension()?.to_str()? == "pmtiles").then_some(path)
    })
    .collect()
}
