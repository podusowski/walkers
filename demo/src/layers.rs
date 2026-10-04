use std::{collections::BTreeMap, path::PathBuf};

use egui::Context;
#[cfg(feature = "pmtiles")]
use walkers::PmTiles;
#[cfg(feature = "mvt")]
use walkers::Style;
use walkers::{HttpOptions, HttpTiles, LocalTiles, Tiles, sources::TileSource};

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
pub struct Layers {
    pub available: BTreeMap<String, Vec<TilesKind>>,
    pub selected: String,
    #[cfg(feature = "pmtiles")]
    pub have_some_pmtiles: bool,
}

pub(crate) fn layers(egui_ctx: Context) -> Layers {
    let mut layers = Layers {
        selected: "OpenStreetMap".to_string(),
        ..Default::default()
    };

    insert_raster_layers(&mut layers, &egui_ctx);
    insert_local_layers(&mut layers, &egui_ctx);
    insert_mapbox_layers(&mut layers, &egui_ctx);
    // Each of these overrides `selected`, so the last available one becomes the default.
    #[cfg(feature = "mvt")]
    insert_openfreemap_layers(&mut layers, &egui_ctx);
    #[cfg(feature = "pmtiles")]
    insert_pmtiles_layers(&mut layers, &egui_ctx);

    layers
}

fn http<S>(source: S, egui_ctx: &Context) -> TilesKind
where
    S: TileSource + Sync + Send + 'static,
{
    TilesKind::Http(HttpTiles::with_options(
        source,
        http_options(),
        egui_ctx.to_owned(),
    ))
}

fn insert_raster_layers(layers: &mut Layers, egui_ctx: &Context) {
    layers.available.insert(
        "OpenStreetMap".to_string(),
        vec![http(walkers::sources::OpenStreetMap, egui_ctx)],
    );

    layers.available.insert(
        "Geoportal".to_string(),
        vec![http(walkers::sources::Geoportal, egui_ctx)],
    );

    layers.available.insert(
        "OpenStreetMapWithGeoportal".to_string(),
        vec![
            http(walkers::sources::OpenStreetMap, egui_ctx),
            http(walkers::sources::Geoportal, egui_ctx),
        ],
    );
}

fn insert_local_layers(layers: &mut Layers, egui_ctx: &Context) {
    #[allow(deprecated)]
    layers.available.insert(
        "LocalTiles".to_string(),
        vec![TilesKind::Local(LocalTiles::new(
            PathBuf::from_iter(&[env!("CARGO_MANIFEST_DIR"), "assets"]),
            egui_ctx.to_owned(),
        ))],
    );
}

#[cfg(feature = "mvt")]
fn insert_openfreemap_layers(layers: &mut Layers, egui_ctx: &Context) {
    let styles = [
        ("OpenFreeMap", Style::openfreemap_bright()),
        (
            "OpenFreeMap (Walkers basemap light)",
            Style::openmaptiles_basemap_light(),
        ),
        (
            "OpenFreeMap (Walkers basemap dark)",
            Style::openmaptiles_basemap_dark(),
        ),
    ];

    for (name, style) in styles {
        layers.available.insert(
            name.to_string(),
            vec![TilesKind::Http(HttpTiles::with_options_and_style(
                walkers::sources::OpenFreeMap,
                http_options(),
                style,
                egui_ctx.to_owned(),
            ))],
        );
    }

    layers.selected = "OpenFreeMap (Walkers basemap dark)".to_string();
}

#[cfg(feature = "pmtiles")]
fn insert_pmtiles_layers(layers: &mut Layers, egui_ctx: &Context) {
    let pmtiles = find_pmtiles_files();
    layers.have_some_pmtiles = !pmtiles.is_empty();

    for path in pmtiles {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let pmtiles = |style| {
            TilesKind::PmTiles(PmTiles::with_style(
                path.clone(),
                style,
                egui_ctx.to_owned(),
            ))
        };

        layers
            .available
            .insert(name.clone(), vec![pmtiles(Style::protomaps_dark())]);
        layers.available.insert(
            format!("{name} (Protomaps Dark Vis)"),
            vec![pmtiles(Style::protomaps_dark_vis())],
        );
        layers.available.insert(
            format!("{name} (Protomaps Light)"),
            vec![pmtiles(Style::protomaps_light())],
        );
        layers.available.insert(
            format!("{name} (Walkers basemap light)"),
            vec![pmtiles(Style::protomaps_basemap_light())],
        );
        layers.available.insert(
            format!("{name} (Walkers basemap dark)"),
            vec![pmtiles(Style::protomaps_basemap_dark())],
        );
        layers.available.insert(
            format!("{name}WithGeoportal"),
            vec![
                pmtiles(Style::protomaps_dark()),
                http(walkers::sources::Geoportal, egui_ctx),
            ],
        );

        layers.selected = format!("{name} (Walkers basemap dark)");
    }
}

/// Mapbox is shown only if an access token was passed at compile time. May or may not be what you
/// want to do, potentially loading it from application settings instead.
fn insert_mapbox_layers(layers: &mut Layers, egui_ctx: &Context) {
    let Some(token) = std::option_env!("MAPBOX_ACCESS_TOKEN") else {
        return;
    };

    let styles = [
        (
            "MapboxStreets",
            walkers::sources::MapboxStyle::Streets,
            false,
        ),
        (
            "MapboxSatellite",
            walkers::sources::MapboxStyle::Satellite,
            true,
        ),
    ];

    for (name, style, high_resolution) in styles {
        let source = walkers::sources::Mapbox {
            style,
            access_token: token.to_string(),
            high_resolution,
        };
        layers
            .available
            .insert(name.to_string(), vec![http(source, egui_ctx)]);
    }
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
