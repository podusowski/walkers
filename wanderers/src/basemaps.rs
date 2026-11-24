//! Maps the places can be drawn on top of.

use std::path::{Path, PathBuf};

use walkers::{
    DemEncoding, Float, HttpOptions, HttpSource, Paint, Palette, PmTilesSource, Style, StyleLayer,
    StyledSources, Waymark, json, sources,
};
use walkers_extras::{Basemaps, pmtiles_files};

/// Tiles are cached between runs, so that the tile server is not hit more than needed.
fn http_options() -> HttpOptions {
    HttpOptions {
        cache: cache_dir(),
        ..Default::default()
    }
}

fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::home_dir().map(|home| home.join(".cache")))?;
    Some(base.join("wanderers").join("tiles"))
}

/// Maps the places can be drawn on top of. PMTiles only show up when there is a file for them, and
/// Mapbox only when an access token was given at compile time.
pub fn basemaps(egui_ctx: egui::Context) -> Basemaps {
    let mut basemaps = Basemaps::new(egui_ctx);

    basemaps.add("OpenStreetMap", |ctx| {
        http(sources::OpenStreetMap, Style::default(), ctx)
    });
    basemaps.add("OpenStreetMap with Geoportal", openstreetmap_with_geoportal);
    basemaps.add("OpenTopoMap", |ctx| {
        http(
            sources::OpenTopoMap(sources::OpenTopoServer::A),
            Style::default(),
            ctx,
        )
    });
    basemaps.add("Geoportal", |ctx| {
        http(sources::Geoportal, Style::default(), ctx)
    });
    basemaps.add("OpenFreeMap (light)", |ctx| {
        http(
            sources::OpenFreeMap,
            Waymark::new(Palette::light()).build(),
            ctx,
        )
    });
    basemaps.add("OpenFreeMap (dark)", |ctx| {
        http(
            sources::OpenFreeMap,
            Waymark::new(Palette::dark()).build(),
            ctx,
        )
    });
    basemaps.add("OpenFreeMap (bright)", |ctx| {
        http(sources::OpenFreeMap, Style::openfreemap_bright(), ctx)
    });
    basemaps.add("OpenFreeMap with relief (light)", |ctx| {
        relief(Palette::light(), ctx)
    });
    basemaps.add("OpenFreeMap with relief (dark)", |ctx| {
        relief(Palette::dark(), ctx)
    });
    basemaps.add("OpenFreeMap over Geoportal", openfreemap_over_geoportal);

    add_mapbox_basemaps(&mut basemaps);

    // See `just openfreemap-dolnoslaskie` and `just protomaps-dolnoslaskie` in `walkers` for
    // ways of getting a file. A file follows one schema, so only some of these draw anything:
    // Waymark and OpenFreeMap's style draw OpenMapTiles, and Protomaps' styles draw Protomaps.
    let styles = [
        (
            "light",
            (|| Waymark::new(Palette::light()).build()) as fn() -> Style,
        ),
        ("dark", || Waymark::new(Palette::dark()).build()),
        ("bright", Style::openfreemap_bright),
        ("Protomaps dark", Style::protomaps_dark),
        ("Protomaps dark vis", Style::protomaps_dark_vis),
        ("Protomaps light", Style::protomaps_light),
    ];

    for path in pmtiles_files(Path::new(".")) {
        let name = path.file_stem().unwrap_or_default().to_string_lossy();

        for (shade, style) in styles {
            let path = path.to_owned();
            basemaps.add(format!("{name} ({shade})"), move |ctx| {
                pmtiles(&path, style(), ctx)
            });
        }

        let path = path.to_owned();
        basemaps.add(
            format!("{name} with Geoportal (Protomaps dark)"),
            move |ctx| pmtiles_with_geoportal(&path, ctx),
        );
    }

    basemaps
}

/// A tile server's tiles drawn with `style`, or as they are when it has no layers.
fn http(
    source: impl sources::TileUrls + Send + Sync + 'static,
    style: Style,
    egui_ctx: &egui::Context,
) -> StyledSources {
    StyledSources::new(style, egui_ctx.to_owned()).with_tiled_source("", fetched(source, egui_ctx))
}

/// Tiles of a tile server, fetched with [`http_options`].
fn fetched(
    source: impl sources::TileUrls + Send + Sync + 'static,
    egui_ctx: &egui::Context,
) -> HttpSource {
    HttpSource::new(source, http_options(), egui_ctx.to_owned())
}

fn pmtiles(path: &Path, style: Style, egui_ctx: &egui::Context) -> StyledSources {
    let source = PmTilesSource::new(path, egui_ctx.to_owned());
    StyledSources::new(style, egui_ctx.to_owned()).with_tiled_source("", source)
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

/// OpenStreetMap, with Geoportal's aerial photos faintly over it.
fn openstreetmap_with_geoportal(egui_ctx: &egui::Context) -> StyledSources {
    let style = Style {
        layers: vec![raster_layer("osm", 1.0), raster_layer("geoportal", 0.25)],
        ..Default::default()
    };

    StyledSources::new(style, egui_ctx.to_owned())
        .with_tiled_source("osm", fetched(sources::OpenStreetMap, egui_ctx))
        .with_tiled_source("geoportal", fetched(sources::Geoportal, egui_ctx))
}

/// Geoportal's aerial photos, with OpenFreeMap's roads, buildings and labels over them.
fn openfreemap_over_geoportal(egui_ctx: &egui::Context) -> StyledSources {
    let mut style = Waymark::new(Palette::light()).build();
    style
        .layers
        .insert(ground(&style), raster_layer("geoportal", 1.0));

    StyledSources::new(style, egui_ctx.to_owned())
        .with_tiled_source("openmaptiles", fetched(sources::OpenFreeMap, egui_ctx))
        .with_tiled_source("geoportal", fetched(sources::Geoportal, egui_ctx))
}

/// Where layers go to be over land and water, but under everything else.
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

/// OpenFreeMap in the colors of `palette`, with Mapterhorn's elevation shaded into relief and
/// traced into contour lines.
fn relief(palette: Palette, egui_ctx: &egui::Context) -> StyledSources {
    let style = Waymark::new(palette)
        .hillshade("terrain")
        .contours("terrain")
        .build();

    StyledSources::new(style, egui_ctx.to_owned())
        .with_tiled_source("openmaptiles", fetched(sources::OpenFreeMap, egui_ctx))
        .with_tiled_source(
            "terrain",
            fetched(sources::Mapterhorn, egui_ctx).with_elevation(DemEncoding::Terrarium),
        )
}

/// A Protomaps file, with Geoportal's aerial photos faintly over it.
fn pmtiles_with_geoportal(path: &Path, egui_ctx: &egui::Context) -> StyledSources {
    let mut style = Style::protomaps_dark();
    style.layers.push(raster_layer("geoportal", 0.25));

    StyledSources::new(style, egui_ctx.to_owned())
        .with_tiled_source("protomaps", PmTilesSource::new(path, egui_ctx.to_owned()))
        .with_tiled_source("geoportal", fetched(sources::Geoportal, egui_ctx))
}

/// Mapbox' streets and satellite photos, if an access token was given at compile time.
fn add_mapbox_basemaps(basemaps: &mut Basemaps) {
    let Some(token) = std::option_env!("MAPBOX_ACCESS_TOKEN") else {
        return;
    };

    let styles = [
        ("Mapbox streets", sources::MapboxStyle::Streets, false),
        ("Mapbox satellite", sources::MapboxStyle::Satellite, true),
    ];

    for (name, style, high_resolution) in styles {
        basemaps.add(name, move |ctx| {
            let source = sources::Mapbox {
                style,
                access_token: token.to_owned(),
                high_resolution,
            };
            http(source, Style::default(), ctx)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vector maps come in both shades, and each is its own entry in the picker.
    #[test]
    fn every_shade_of_a_vector_basemap_is_offered_separately() {
        let basemaps = basemaps(egui::Context::default());
        let names: Vec<_> = basemaps.names().collect();

        assert!(names.contains(&"OpenFreeMap (light)"));
        assert!(names.contains(&"OpenFreeMap (dark)"));

        // Raster ones have nothing to shade, so they stay single.
        assert_eq!(
            names
                .iter()
                .filter(|name| **name == "OpenStreetMap")
                .count(),
            1
        );
    }

    /// Two entries which look the same in the picker would be indistinguishable to click.
    #[test]
    fn basemap_names_are_unique() {
        let basemaps = basemaps(egui::Context::default());
        let names: Vec<_> = basemaps.names().collect();
        let unique: std::collections::HashSet<_> = names.iter().collect();

        assert_eq!(names.len(), unique.len(), "duplicate names in {names:?}");
    }
}
