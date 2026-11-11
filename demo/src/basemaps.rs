use egui::Context;
#[cfg(feature = "pmtiles")]
use walkers::PmTilesSource;
#[cfg(feature = "mvt")]
use walkers::{
    DemEncoding,
    sources::{Mapterhorn, OpenFreeMap},
};
use walkers::{
    Float, HttpOptions, HttpSource, Paint, Style, StyleLayer, StyledSources, json,
    sources::{Geoportal, OpenStreetMap, OpenTopoMap, OpenTopoServer, TileUrls},
};
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

fn http(source: impl TileUrls + Sync + Send + 'static, ctx: &Context) -> HttpSource {
    HttpSource::new(source, http_options(), ctx.to_owned())
}

/// Tiles of a raster tile server, drawn as they are.
fn raster(source: impl TileUrls + Sync + Send + 'static, ctx: &Context) -> StyledSources {
    StyledSources::new(Style::default(), ctx.to_owned())
        .with_tiled_source("raster", http(source, ctx))
}

/// Draws the raster tiles of `source`, `opacity` of them showing.
fn raster_layer(source: &str, opacity: f32) -> StyleLayer {
    StyleLayer::Raster {
        source: Some(source.to_owned()),
        paint: Paint {
            raster_opacity: Some(Float(json!(opacity))),
            ..Default::default()
        },
    }
}

fn add_raster_basemaps(basemaps: &mut Basemaps) {
    basemaps.add("OpenStreetMap", |ctx| raster(OpenStreetMap, ctx));
    basemaps.add("OpenTopoMap", |ctx| {
        raster(OpenTopoMap(OpenTopoServer::A), ctx)
    });
    basemaps.add("Geoportal", |ctx| raster(Geoportal, ctx));

    // Sources can be stacked, each layer with its own opacity.
    basemaps.add("OpenStreetMapWithGeoportal", |ctx| {
        let style = Style {
            layers: vec![raster_layer("osm", 1.0), raster_layer("geoportal", 0.25)],
        };
        StyledSources::new(style, ctx.to_owned())
            .with_tiled_source("osm", http(OpenStreetMap, ctx))
            .with_tiled_source("geoportal", http(Geoportal, ctx))
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
            StyledSources::new(style(), ctx.to_owned())
                .with_tiled_source("openmaptiles", http(OpenFreeMap, ctx))
        });
    }

    basemaps.add("OpenFreeMap over Geoportal", hybrid);
    basemaps.add("OpenFreeMap with relief (Walkers light)", |ctx| {
        relief(Style::openmaptiles_basemap_light(), ctx)
    });
    basemaps.add("OpenFreeMap with relief (Walkers dark)", |ctx| {
        relief(Style::openmaptiles_basemap_dark(), ctx)
    });

    basemaps.select("OpenFreeMap (Walkers dark)");
}

/// Geoportal's aerial photos, with OpenFreeMap's roads, buildings and labels over them. One style
/// drawing from two sources.
#[cfg(feature = "mvt")]
fn hybrid(ctx: &Context) -> StyledSources {
    let mut style = Style::openmaptiles_basemap_light();
    style
        .layers
        .insert(ground(&style), raster_layer("geoportal", 1.0));

    StyledSources::new(style, ctx.to_owned())
        .with_tiled_source("openmaptiles", http(OpenFreeMap, ctx))
        .with_tiled_source("geoportal", http(Geoportal, ctx))
}

/// OpenFreeMap drawn with `style`, and Mapterhorn's elevation shaded into relief between its land
/// and its roads.
#[cfg(feature = "mvt")]
fn relief(mut style: Style, ctx: &Context) -> StyledSources {
    style.layers.insert(
        ground(&style),
        StyleLayer::Hillshade {
            source: Some("terrain".to_owned()),
            paint: Paint::default(),
        },
    );

    StyledSources::new(style, ctx.to_owned())
        .with_tiled_source("openmaptiles", http(OpenFreeMap, ctx))
        .with_tiled_source(
            "terrain",
            http(Mapterhorn, ctx).with_elevation(DemEncoding::Terrarium),
        )
}

/// Where layers go to be over land and water, but under everything else.
#[cfg(feature = "mvt")]
fn ground(style: &Style) -> usize {
    style
        .layers
        .iter()
        .rposition(|layer| {
            matches!(layer, StyleLayer::Fill { source_layer, .. }
                if source_layer.matches("landuse") || source_layer.matches("water"))
        })
        .map_or(0, |index| index + 1)
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

        for (suffix, style) in styles {
            let path = path.clone();
            basemaps.add(format!("{name}{suffix}"), move |ctx| {
                StyledSources::new(style(), ctx.to_owned())
                    .with_tiled_source("protomaps", PmTilesSource::new(&path, ctx.to_owned()))
            });
        }

        let with_geoportal = path.clone();
        basemaps.add(format!("{name}WithGeoportal"), move |ctx| {
            let mut style = Style::protomaps_dark();
            style.layers.push(raster_layer("geoportal", 0.25));

            StyledSources::new(style, ctx.to_owned())
                .with_tiled_source(
                    "protomaps",
                    PmTilesSource::new(&with_geoportal, ctx.to_owned()),
                )
                .with_tiled_source("geoportal", http(Geoportal, ctx))
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
            raster(source, ctx)
        });
    }
}
