use egui::Context;
#[cfg(feature = "pmtiles")]
use walkers::PmTiles;
use walkers::{
    HttpOptions, HttpTiles, MercatorProjection, Tiles,
    sources::{Geoportal, OpenStreetMap, OpenTopoMap, OpenTopoServer, TileSource},
};
#[cfg(feature = "mvt")]
use walkers::{Style, sources::OpenFreeMap};
use walkers_extras::Basemaps;

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

pub(crate) fn basemaps(egui_ctx: Context) -> Basemaps {
    let mut basemaps = Basemaps::new(egui_ctx);

    add_raster_basemaps(&mut basemaps);
    add_mapbox_basemaps(&mut basemaps);
    // Each of these selects one of its own, so the last available one becomes the default.
    #[cfg(feature = "mvt")]
    add_openfreemap_basemaps(&mut basemaps);
    #[cfg(feature = "pmtiles")]
    add_pmtiles_basemaps(&mut basemaps);

    basemaps
}

fn http<S>(source: S, egui_ctx: &Context) -> Box<dyn Tiles<Projection = MercatorProjection>>
where
    S: TileSource<Projection = MercatorProjection> + Sync + Send + 'static,
{
    Box::new(HttpTiles::with_options(
        source,
        http_options(),
        egui_ctx.to_owned(),
    ))
}

fn add_raster_basemaps(basemaps: &mut Basemaps) {
    basemaps.add("OpenStreetMap", |ctx| vec![(http(OpenStreetMap, ctx), 1.0)]);

    basemaps.add("OpenTopoMap", |ctx| {
        vec![(http(OpenTopoMap(OpenTopoServer::A), ctx), 1.0)]
    });

    basemaps.add("Geoportal", |ctx| vec![(http(Geoportal, ctx), 1.0)]);

    // Layers can be stacked, each with its own transparency.
    basemaps.add("OpenStreetMapWithGeoportal", |ctx| {
        vec![
            (http(OpenStreetMap, ctx), 1.0),
            (http(Geoportal, ctx), 0.25),
        ]
    });
}

#[cfg(feature = "mvt")]
fn add_openfreemap_basemaps(basemaps: &mut Basemaps) {
    let styles = [
        ("OpenFreeMap", Style::openfreemap_bright as fn() -> Style),
        (
            "OpenFreeMap (Walkers light)",
            Style::openmaptiles_basemap_light,
        ),
        (
            "OpenFreeMap (Walkers dark)",
            Style::openmaptiles_basemap_dark,
        ),
    ];

    for (name, style) in styles {
        basemaps.add(name, move |ctx| {
            let tiles = HttpTiles::with_options_and_style(
                OpenFreeMap,
                http_options(),
                style(),
                ctx.to_owned(),
            );
            vec![(
                Box::new(tiles) as Box<dyn Tiles<Projection = MercatorProjection>>,
                1.0,
            )]
        });
    }

    basemaps.select("OpenFreeMap (Walkers dark)");
}

#[cfg(feature = "pmtiles")]
fn add_pmtiles_basemaps(basemaps: &mut Basemaps) {
    let styles = [
        ("", Style::protomaps_dark as fn() -> Style),
        (" (Protomaps Dark Vis)", Style::protomaps_dark_vis),
        (" (Protomaps Light)", Style::protomaps_light),
        (" (Walkers light)", Style::protomaps_basemap_light),
        (" (Walkers dark)", Style::protomaps_basemap_dark),
    ];

    for path in walkers_extras::pmtiles_files(std::path::Path::new(".")) {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();

        let pmtiles = move |style: fn() -> Style,
                            ctx: &Context|
              -> Box<dyn Tiles<Projection = MercatorProjection>> {
            Box::new(PmTiles::with_style(
                path.clone(),
                MercatorProjection,
                style(),
                ctx.to_owned(),
            ))
        };

        for (suffix, style) in styles {
            let pmtiles = pmtiles.clone();
            basemaps.add(format!("{name}{suffix}"), move |ctx| {
                vec![(pmtiles(style, ctx), 1.0)]
            });
        }

        basemaps.add(format!("{name}WithGeoportal"), move |ctx| {
            vec![
                (pmtiles(Style::protomaps_dark, ctx), 1.0),
                (http(Geoportal, ctx), 0.25),
            ]
        });

        basemaps.select(&format!("{name} (Walkers dark)"));
    }
}

/// Mapbox is shown only if an access token was passed at compile time. May or may not be what you
/// want to do, potentially loading it from application settings instead.
fn add_mapbox_basemaps(basemaps: &mut Basemaps) {
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
        basemaps.add(name, move |ctx| {
            let source = walkers::sources::Mapbox {
                style,
                access_token: token.to_string(),
                high_resolution,
            };
            vec![(http(source, ctx), 1.0)]
        });
    }
}
