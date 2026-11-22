//! A basemap for OpenFreeMap, or anything else following the OpenMapTiles schema.
//!
//! <https://openmaptiles.org/schema/>

use super::{
    Color, Dasharray, Filter, Float, Layout, Paint, Style, StyleLayer, Value,
    exponential_zoom_interpolation, json, linear_zoom_interpolation,
};

/// What a style source of OpenMapTiles is called, as MapLibre styles usually call it.
const SOURCE: &str = "openmaptiles";

impl Style {
    /// Light Walkers style for OpenMapTiles schema.
    pub fn openmaptiles_basemap_light() -> Self {
        build(&LIGHT)
    }

    /// Dark Walkers style for OpenMapTiles schema.
    pub fn openmaptiles_basemap_dark() -> Self {
        build(&DARK)
    }
}

/// Text size for a city, town or village label, growing with zoom and with how big the place is.
///
/// OpenMapTiles keeps `city`, `town` and `village` apart as classes. Its `rank` is a placement
/// priority counting *down* with importance — Berlin is 2, Wrocław 7, and the villages around it
/// 11 to 15 — so it is the class, not the rank, which sizes a label.
fn settlement_text_size() -> Float {
    let sizes = |city: u32, town: u32, village: u32| {
        json!([
            "match",
            ["get", "class"],
            "city",
            city,
            "town",
            town,
            village
        ])
    };

    Float(json!([
        "interpolate",
        ["linear"],
        ["zoom"],
        2,
        sizes(13, 8, 8),
        4,
        sizes(15, 10, 10),
        6,
        sizes(17, 13, 11),
        8,
        sizes(18, 14, 11),
        10,
        sizes(20, 15, 12),
        15,
        sizes(22, 17, 13)
    ]))
}

struct Palette {
    background: &'static str,
    rail: &'static str,
    rail_tie: &'static str,
    tram: &'static str,
    forest: &'static str,
    urban_green: &'static str,
    pier: &'static str,
    tunnel_casing: &'static str,
    casing: &'static str,
    landuse_dark: &'static str,
    bridge: &'static str,
    structure: &'static str,
    path: &'static str,
    cycleway: &'static str,
    muted: &'static str,
    highway: &'static str,
    road: &'static str,
    label: &'static str,
    locality_text: &'static str,
    water: &'static str,
    water_label: &'static str,
    water_label_halo: &'static str,
    station: &'static str,
    peak: &'static str,
    hospital: &'static str,
}

const DARK: Palette = Palette {
    background: "#000000",
    rail_tie: "#6d6d6d",
    tram: "#2d2d2d",
    rail: "#000000",
    forest: "#061009",
    urban_green: "#000c00",
    pier: "#0a0a0a",
    tunnel_casing: "#101010",
    casing: "#141414",
    landuse_dark: "#191919",
    bridge: "#1f1f1f",
    structure: "#292929",
    path: "#5a4a3a",
    cycleway: "#74463a",
    muted: "#333333",
    highway: "#352121",
    road: "#464646",
    label: "#a0a0a0",
    locality_text: "#999999",
    water: "#161e31",
    water_label: "#134164",
    water_label_halo: "#0d1526",
    station: "#98b07a",
    peak: "#b59a6a",
    hospital: "#c47c7c",
};

const LIGHT: Palette = Palette {
    background: "#f2f2f0",
    rail_tie: "#000000",
    tram: "#000000",
    rail: "#ffffff",
    forest: "#86a180",
    urban_green: "#e2f0df",
    pier: "#8c8c8c",
    tunnel_casing: "#595959",
    casing: "#e9e9e9",
    landuse_dark: "#e0e0e0",
    bridge: "#bfbfbf",
    structure: "#5b5b5b",
    path: "#4c3316",
    cycleway: "#b86b6b",
    muted: "#b3b3b3",
    highway: "#bb5f5f",
    road: "#4a4a4a",
    label: "#000000",
    locality_text: "#1a1a1a",
    water: "#88b2e2",
    water_label: "#2f6690",
    water_label_halo: "#ffffff",
    station: "#3f5934",
    peak: "#6b5330",
    hospital: "#8f2f2f",
};

/// OpenMapTiles spreads land use over three layers.
const LANDUSE: [&str; 3] = ["landcover", "landuse", "park"];

const ROAD_WIDTH: [(f64, f64); 5] = [
    (3.0, 0.0),
    (6.0, 0.66),
    (12.0, 0.96),
    (15.0, 2.0),
    (18.0, 11.0),
];

const PATH_FACTOR: f64 = 0.5;
const MINOR_ROAD_FACTOR: f64 = 1.0;
const MAJOR_ROAD_FACTOR: f64 = 1.5;
const MOTORWAY_FACTOR: f64 = 2.5;
const RAIL_FACTOR: f64 = 0.5;
const TRAM_FACTOR: f64 = 0.3;

const HALO_WIDTH: f64 = 1.0;

const SMALL_TEXT: f64 = 10.0;
const TEXT: f64 = 12.0;
const LARGE_TEXT: f64 = 14.0;

/// `icon-size` of an icon standing for its feature as a label of `TEXT` would, such as a peak.
const ICON: f64 = 1.0;
/// `icon-size` of an icon among `SMALL_TEXT` labels, such as a bus stop.
const SMALL_ICON: f64 = SMALL_TEXT / TEXT;

fn road_width(factor: f64) -> Float {
    exponential_zoom_interpolation(1.6, &ROAD_WIDTH.map(|(zoom, width)| (zoom, width * factor)))
}

/// Width of a casing: its line, with a pixel either side. That is never wider than the line, so
/// that a line too thin to see does not get a casing which is not.
fn casing_width(line: Value) -> Float {
    Float(json!(["+", line, ["*", 2, ["min", 1, line]]]))
}

/// Paths and tracks, dashed in `color`, and cycleways, solid in `cycleway_color`.
fn path_paint(color: &str, cycleway_color: &str) -> Paint {
    Paint {
        line_color: Some(Color(json!([
            "match",
            ["get", "subclass"],
            "cycleway",
            cycleway_color,
            color
        ]))),
        line_width: Some(road_width(PATH_FACTOR)),
        line_dasharray: Some(Dasharray(json!([
            "match",
            ["get", "subclass"],
            "cycleway",
            ["literal", []],
            ["literal", [6, 2]]
        ]))),
        ..Default::default()
    }
}

fn build(palette: &Palette) -> Style {
    // Parking aisles and driveways only from up close, as they cover car parks and housing
    // estates in a mesh of lines.
    let service_road_shown = json!([
        "any",
        ["!", ["in", "service", "parking_aisle", "driveway"]],
        [">=", ["zoom"], 16]
    ]);

    // Service roads are minor roads, as narrow as paths.
    let minor_road_width = json!([
        "match",
        ["get", "class"],
        "service",
        road_width(PATH_FACTOR).0,
        road_width(MINOR_ROAD_FACTOR).0
    ]);

    // Ramps, such as motorway exits, are drawn with their road, as wide as minor roads.
    let with_ramps = |factor| {
        json!([
            "match",
            ["get", "ramp"],
            1,
            road_width(MINOR_ROAD_FACTOR).0,
            road_width(factor).0
        ])
    };
    let major_road_width = with_ramps(MAJOR_ROAD_FACTOR);
    let motorway_width = with_ramps(MOTORWAY_FACTOR);

    let layers = vec![
        // background
        StyleLayer::Background {
            source: Some(SOURCE.into()),
            paint: Paint {
                background_color: Some(Color(json!(palette.background))),
                ..Default::default()
            },
        },
        // landuse_park
        StyleLayer::Fill {
            source: Some(SOURCE.into()),
            source_layer: LANDUSE.into(),
            filter: Some(Filter(json!([
                "in",
                "class",
                "national_park",
                "park",
                "public_park",
                "nature_reserve",
                "cemetery",
                "forest",
                "golf_course",
                "wood",
                "farmland",
                "scrub",
                "grassland",
                "grass",
                "military",
                "naval_base",
                "airfield"
            ]))),
            paint: Paint {
                fill_opacity: Some(Float(json!(0.5))),
                fill_color: Some(Color(json!(palette.forest))),
                ..Default::default()
            },
        },
        // landuse_urban_green
        StyleLayer::Fill {
            source: Some(SOURCE.into()),
            source_layer: LANDUSE.into(),
            filter: Some(Filter(json!([
                "in",
                "class",
                "allotments",
                "village_green",
                "playground"
            ]))),
            paint: Paint {
                fill_color: Some(Color(json!(palette.urban_green))),
                ..Default::default()
            },
        },
        // landuse_hospital
        StyleLayer::Fill {
            source: Some(SOURCE.into()),
            source_layer: LANDUSE.into(),
            filter: Some(Filter(json!(["==", "class", "hospital"]))),
            paint: Paint {
                fill_color: Some(Color(json!(palette.landuse_dark))),
                ..Default::default()
            },
        },
        // landuse_beach
        StyleLayer::Fill {
            source: Some(SOURCE.into()),
            source_layer: LANDUSE.into(),
            filter: Some(Filter(json!(["in", "class", "beach"]))),
            paint: Paint {
                fill_color: Some(Color(json!(palette.bridge))),
                ..Default::default()
            },
        },
        // landuse_zoo
        StyleLayer::Fill {
            source: Some(SOURCE.into()),
            source_layer: LANDUSE.into(),
            filter: Some(Filter(json!(["in", "class", "zoo"]))),
            paint: Paint {
                fill_color: Some(Color(json!(palette.landuse_dark))),
                ..Default::default()
            },
        },
        // landuse_aerodrome
        StyleLayer::Fill {
            source: Some(SOURCE.into()),
            source_layer: LANDUSE.into(),
            filter: Some(Filter(json!(["in", "class", "aerodrome"]))),
            paint: Paint {
                fill_color: Some(Color(json!(palette.landuse_dark))),
                ..Default::default()
            },
        },
        // roads_runway
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!(["==", "subclass", "runway"]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.muted))),
                line_width: Some(linear_zoom_interpolation(&[
                    (10.0, 0.0),
                    (12.0, 4.0),
                    (18.0, 30.0),
                ])),
                ..Default::default()
            },
        },
        // roads_taxiway
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!(["==", "subclass", "taxiway"]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.muted))),
                line_width: Some(linear_zoom_interpolation(&[
                    (13.0, 0.0),
                    (13.5, 1.0),
                    (15.0, 6.0),
                ])),
                ..Default::default()
            },
        },
        // landuse_runway
        StyleLayer::Fill {
            source: Some(SOURCE.into()),
            source_layer: LANDUSE.into(),
            filter: Some(Filter(json!(["in", "class", "runway", "taxiway"]))),
            paint: Paint {
                fill_color: Some(Color(json!(palette.muted))),
                ..Default::default()
            },
        },
        // water
        StyleLayer::Fill {
            source: Some(SOURCE.into()),
            source_layer: "water".into(),
            filter: Some(Filter(json!(["==", ["geometry-type"], "Polygon"]))),
            paint: Paint {
                fill_color: Some(Color(json!(palette.water))),
                ..Default::default()
            },
        },
        // water_stream, and canals, which are wider
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "waterway".into(),
            filter: Some(Filter(json!(["in", "class", "stream", "canal"]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.water))),
                line_width: Some(Float(json!([
                    "match",
                    ["get", "class"],
                    "canal",
                    linear_zoom_interpolation(&[(12.0, 1.0), (18.0, 8.0)]).0,
                    linear_zoom_interpolation(&[(12.0, 1.0), (18.0, 4.0)]).0
                ]))),
                ..Default::default()
            },
        },
        // water_river
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "waterway".into(),
            filter: Some(Filter(json!(["in", "class", "river"]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.water))),
                line_width: Some(linear_zoom_interpolation(&[
                    (9.0, 0.0),
                    (9.5, 1.0),
                    (18.0, 12.0),
                ])),
                ..Default::default()
            },
        },
        // landuse_pedestrian
        StyleLayer::Fill {
            source: Some(SOURCE.into()),
            source_layer: LANDUSE.into(),
            filter: Some(Filter(json!(["in", "class", "pedestrian", "dam"]))),
            paint: Paint {
                fill_color: Some(Color(json!(palette.landuse_dark))),
                ..Default::default()
            },
        },
        // landuse_pier
        StyleLayer::Fill {
            source: Some(SOURCE.into()),
            source_layer: LANDUSE.into(),
            filter: Some(Filter(json!(["==", "class", "pier"]))),
            paint: Paint {
                fill_color: Some(Color(json!(palette.pier))),
                ..Default::default()
            },
        },
        // roads_tunnels_other_casing
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!([
                "all",
                ["==", "brunnel", "tunnel"],
                ["in", "class", "path", "track"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.tunnel_casing))),
                line_dasharray: Some(Dasharray(json!([2, 2]))),
                line_width: Some(casing_width(road_width(PATH_FACTOR).0)),
                ..Default::default()
            },
        },
        // roads_tunnels_minor_casing
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!([
                "all",
                ["==", "brunnel", "tunnel"],
                ["in", "class", "minor", "tertiary", "service"],
                service_road_shown.clone()
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.tunnel_casing))),
                line_dasharray: Some(Dasharray(json!([2, 2]))),
                line_width: Some(casing_width(minor_road_width.clone())),
                ..Default::default()
            },
        },
        // roads_tunnels_major_casing
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!([
                "all",
                ["==", "brunnel", "tunnel"],
                ["in", "class", "primary", "secondary"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.tunnel_casing))),
                line_dasharray: Some(Dasharray(json!([2, 2]))),
                line_width: Some(casing_width(major_road_width.clone())),
                ..Default::default()
            },
        },
        // roads_tunnels_highway_casing
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!([
                "all",
                ["==", "brunnel", "tunnel"],
                ["in", "class", "motorway", "trunk"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.tunnel_casing))),
                line_dasharray: Some(Dasharray(json!([2, 2]))),
                line_width: Some(casing_width(motorway_width.clone())),
                ..Default::default()
            },
        },
        // roads_tunnels_other
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!([
                "all",
                ["==", "brunnel", "tunnel"],
                ["in", "class", "path", "track"]
            ]))),
            paint: path_paint(palette.structure, palette.structure),
        },
        // roads_tunnels_minor
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!([
                "all",
                ["==", "brunnel", "tunnel"],
                ["in", "class", "minor", "tertiary", "service"],
                service_road_shown.clone()
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.structure))),
                line_width: Some(Float(minor_road_width.clone())),
                ..Default::default()
            },
        },
        // roads_tunnels_major
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!([
                "all",
                ["==", "brunnel", "tunnel"],
                ["in", "class", "primary", "secondary"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.structure))),
                line_width: Some(Float(major_road_width.clone())),
                ..Default::default()
            },
        },
        // roads_tunnels_highway
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!([
                "all",
                ["==", "brunnel", "tunnel"],
                ["in", "class", "motorway", "trunk"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.structure))),
                line_width: Some(Float(motorway_width.clone())),
                ..Default::default()
            },
        },
        // buildings
        StyleLayer::Fill {
            source: Some(SOURCE.into()),
            source_layer: "building".into(),
            filter: None,
            paint: Paint {
                fill_color: Some(Color(json!(palette.structure))),
                fill_opacity: Some(Float(json!(0.5))),
                ..Default::default()
            },
        },
        // roads_pier
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!(["==", "subclass", "pier"]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.pier))),
                line_width: Some(linear_zoom_interpolation(&[
                    (12.0, 0.0),
                    (12.5, 0.5),
                    (20.0, 16.0),
                ])),
                ..Default::default()
            },
        },
        // roads_other
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!([
                "all",
                ["!=", "brunnel", "tunnel"],
                ["!=", "brunnel", "bridge"],
                ["in", "class", "path", "track"],
                ["!=", "subclass", "pier"]
            ]))),
            paint: path_paint(palette.path, palette.cycleway),
        },
        // roads_minor
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!([
                "all",
                ["!=", "brunnel", "tunnel"],
                ["!=", "brunnel", "bridge"],
                ["in", "class", "minor", "tertiary", "service"],
                service_road_shown.clone()
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.road))),
                line_width: Some(Float(minor_road_width.clone())),
                ..Default::default()
            },
        },
        // roads_major
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!([
                "all",
                ["!=", "brunnel", "tunnel"],
                ["!=", "brunnel", "bridge"],
                ["in", "class", "primary", "secondary"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.road))),
                line_width: Some(Float(major_road_width.clone())),
                ..Default::default()
            },
        },
        // roads_highway
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!([
                "all",
                ["!=", "brunnel", "tunnel"],
                ["!=", "brunnel", "bridge"],
                ["in", "class", "motorway", "trunk"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.highway))),
                line_width: Some(Float(motorway_width.clone())),
                ..Default::default()
            },
        },
        // roads_rail
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!(["==", "class", "rail"]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.rail))),
                line_width: Some(casing_width(road_width(RAIL_FACTOR).0)),
                ..Default::default()
            },
        },
        // roads_rail_inside
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!(["==", "class", "rail"]))),
            paint: Paint {
                //line_opacity: Some(Float(json!(0.5))),
                line_color: Some(Color(json!(palette.rail_tie))),
                line_dasharray: Some(Dasharray(json!([4, 2]))),
                line_width: Some(road_width(RAIL_FACTOR)),
                ..Default::default()
            },
        },
        // roads_tram
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!(["==", "class", "transit"]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.tram))),
                line_width: Some(road_width(TRAM_FACTOR)),
                ..Default::default()
            },
        },
        // boundaries_country
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "boundary".into(),
            filter: Some(Filter(json!(["<=", "admin_level", 2]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.label))),
                line_width: Some(Float(json!(2))),
                line_dasharray: Some(Dasharray(json!([
                    "step",
                    ["zoom"],
                    ["literal", [2, 0]],
                    4,
                    ["literal", [2, 1]]
                ]))),
                ..Default::default()
            },
        },
        // roads_bridges_other
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!([
                "all",
                ["==", "brunnel", "bridge"],
                ["in", "class", "path", "track"]
            ]))),
            paint: path_paint(palette.path, palette.cycleway),
        },
        // roads_bridges_minor
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!([
                "all",
                ["==", "brunnel", "bridge"],
                ["in", "class", "minor", "tertiary", "service"],
                service_road_shown.clone()
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.road))),
                line_width: Some(Float(minor_road_width.clone())),
                ..Default::default()
            },
        },
        // roads_bridges_major
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!([
                "all",
                ["==", "brunnel", "bridge"],
                ["in", "class", "primary", "secondary"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.road))),
                line_width: Some(Float(major_road_width.clone())),
                ..Default::default()
            },
        },
        // roads_bridges_highway
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!([
                "all",
                ["==", "brunnel", "bridge"],
                ["in", "class", "motorway", "trunk"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.highway))),
                line_width: Some(Float(motorway_width.clone())),
                ..Default::default()
            },
        },
        // house_numbers
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "housenumber".into(),
            minzoom: Some(18.0),
            filter: None,
            layout: Layout {
                text_field: Some(json!(["get", "housenumber"])),
                text_size: Some(Float(json!(SMALL_TEXT))),
                ..Default::default()
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.label))),
                text_halo_color: Some(Color(json!(palette.casing))),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // water_waterway_label
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "waterway".into(),
            minzoom: None,
            filter: Some(Filter(json!(["in", "class", "river", "stream"]))),
            layout: Layout {
                text_field: Some(json!(["get", "name"])),
                text_size: Some(Float(json!(TEXT))),
                ..Default::default()
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.water_label))),
                text_halo_color: Some(Color(json!(palette.water_label_halo))),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // roads_labels_minor
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "transportation_name".into(),
            minzoom: None,
            filter: Some(Filter(json!([
                "in", "class", "minor", "tertiary", "path", "service", "track"
            ]))),
            layout: Layout {
                text_field: Some(json!(["get", "name"])),
                text_size: Some(Float(json!(TEXT))),
                ..Default::default()
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.label))),
                text_halo_color: Some(Color(json!(palette.background))),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // water_label_ocean
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "water_name".into(),
            minzoom: None,
            filter: Some(Filter(json!([
                "in", "class", "sea", "ocean", "bay", "strait", "fjord"
            ]))),
            layout: Layout {
                text_field: Some(json!(["get", "name"])),
                text_size: Some(Float(json!(TEXT))),
                ..Default::default()
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.water_label))),
                text_halo_color: Some(Color(json!(palette.water_label_halo))),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // water_label_lakes
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "water_name".into(),
            minzoom: None,
            filter: Some(Filter(json!(["in", "class", "lake", "water"]))),
            layout: Layout {
                text_field: Some(json!(["get", "name"])),
                text_size: Some(Float(json!(TEXT))),
                ..Default::default()
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.water_label))),
                text_halo_color: Some(Color(json!(palette.water_label_halo))),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // roads_labels_junction, motorway exits marked with an arrow, named under it
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "transportation_name".into(),
            minzoom: None,
            filter: Some(Filter(json!(["==", "subclass", "junction"]))),
            layout: Layout {
                icon_image: Some(json!("exit")),
                icon_size: Some(Float(json!(ICON))),
                text_field: Some(json!(["get", "name"])),
                text_size: Some(Float(json!(SMALL_TEXT))),
                text_anchor: Some(json!("top")),
                text_offset: Some(json!([0, 0.8])),
                ..Default::default()
            },
            paint: Some(Paint {
                icon_color: Some(Color(json!(palette.label))),
                icon_halo_color: Some(Color(json!(palette.background))),
                icon_halo_width: Some(Float(json!(HALO_WIDTH))),
                text_color: Some(Color(json!(palette.label))),
                text_halo_color: Some(Color(json!(palette.background))),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // roads_labels_major
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "transportation_name".into(),
            minzoom: None,
            filter: Some(Filter(json!([
                "all",
                ["in", "class", "motorway", "trunk", "primary", "secondary"],
                ["!=", "subclass", "junction"]
            ]))),
            layout: Layout {
                text_field: Some(json!(["get", "name"])),
                text_size: Some(Float(json!(LARGE_TEXT))),
                ..Default::default()
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.label))),
                text_halo_color: Some(Color(json!(palette.background))),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // hospitals, marked with a plus, named under it
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "poi".into(),
            minzoom: Some(14.0),
            filter: Some(Filter(json!([
                "all",
                ["==", "class", "hospital"],
                ["==", "subclass", "hospital"]
            ]))),
            layout: Layout {
                icon_image: Some(json!("hospital")),
                icon_size: Some(Float(json!(ICON))),
                text_field: Some(json!(["get", "name"])),
                text_size: Some(Float(json!(TEXT))),
                text_anchor: Some(json!("top")),
                text_offset: Some(json!([0, 0.8])),
                ..Default::default()
            },
            paint: Some(Paint {
                icon_color: Some(Color(json!(palette.hospital))),
                icon_halo_color: Some(Color(json!(palette.casing))),
                icon_halo_width: Some(Float(json!(HALO_WIDTH))),
                text_color: Some(Color(json!(palette.hospital))),
                text_halo_color: Some(Color(json!(palette.casing))),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // peaks, marked with a mountain, named under it
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "mountain_peak".into(),
            minzoom: None,
            filter: Some(Filter(json!(["in", "class", "peak", "volcano"]))),
            layout: Layout {
                icon_image: Some(json!("peak")),
                icon_size: Some(Float(json!(ICON))),
                text_field: Some(json!([
                    "case",
                    ["!has", "ele"],
                    ["get", "name"],
                    ["!has", "name"],
                    ["concat", ["get", "ele"], " m"],
                    ["concat", ["get", "name"], "\n", ["get", "ele"], " m"]
                ])),
                text_size: Some(Float(json!(TEXT))),
                text_anchor: Some(json!("top")),
                text_offset: Some(json!([0, 0.7])),
                ..Default::default()
            },
            paint: Some(Paint {
                icon_color: Some(Color(json!(palette.peak))),
                icon_halo_color: Some(Color(json!(palette.casing))),
                icon_halo_width: Some(Float(json!(HALO_WIDTH))),
                text_color: Some(Color(json!(palette.peak))),
                text_halo_color: Some(Color(json!(palette.casing))),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // places_subplace
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "place".into(),
            minzoom: None,
            filter: Some(Filter(json!([
                "in",
                "class",
                "neighbourhood",
                "suburb",
                "quarter"
            ]))),
            layout: Layout {
                text_field: Some(json!(["get", "name"])),
                text_size: Some(linear_zoom_interpolation(&[
                    (11.0, SMALL_TEXT),
                    (14.0, LARGE_TEXT),
                    (18.0, 16.0),
                ])),
                ..Default::default()
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.label))),
                text_halo_color: Some(Color(json!(palette.casing))),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // places_region
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "place".into(),
            minzoom: None,
            filter: Some(Filter(json!(["==", "class", "state"]))),
            layout: Layout {
                text_field: Some(json!(["get", "name"])),
                text_size: Some(linear_zoom_interpolation(&[(3.0, 11.0), (7.0, 16.0)])),
                ..Default::default()
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.label))),
                text_halo_color: Some(Color(json!(palette.casing))),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // places_locality
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "place".into(),
            minzoom: None,
            filter: Some(Filter(json!(["in", "class", "city", "town", "village"]))),
            layout: Layout {
                text_field: Some(json!(["get", "name"])),
                text_size: Some(settlement_text_size()),
                // Keeps villages from packing edge to edge, the curve being Protomaps' own.
                text_padding: Some(linear_zoom_interpolation(&[
                    (5.0, 3.0),
                    (8.0, 7.0),
                    (12.0, 11.0),
                ])),
                ..Default::default()
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.locality_text))),
                text_halo_color: Some(Color(json!(palette.casing))),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // places_country
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "place".into(),
            minzoom: None,
            filter: Some(Filter(json!(["==", "class", "country"]))),
            layout: Layout {
                text_field: Some(json!(["get", "name:en"])),
                text_size: Some(Float(json!(18.0))),
                ..Default::default()
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.label))),
                text_halo_color: Some(Color(json!(palette.casing))),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // airports, marked with a plane, named under it
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "aerodrome_label".into(),
            minzoom: Some(10.0),
            filter: Some(Filter(json!(["has", "iata"]))),
            layout: Layout {
                icon_image: Some(json!("airport")),
                icon_size: Some(Float(json!(ICON))),
                text_field: Some(json!(["get", "name"])),
                text_size: Some(Float(json!(TEXT))),
                text_anchor: Some(json!("top")),
                text_offset: Some(json!([0, 0.8])),
                ..Default::default()
            },
            paint: Some(Paint {
                icon_color: Some(Color(json!(palette.station))),
                icon_halo_color: Some(Color(json!(palette.casing))),
                icon_halo_width: Some(Float(json!(HALO_WIDTH))),
                text_color: Some(Color(json!(palette.station))),
                text_halo_color: Some(Color(json!(palette.casing))),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // stations, marked with a train, named under it
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "poi".into(),
            minzoom: None,
            filter: Some(Filter(json!([
                "all",
                ["==", "class", "railway"],
                ["in", "subclass", "station", "halt"]
            ]))),
            layout: Layout {
                icon_image: Some(json!("train")),
                icon_size: Some(Float(json!(ICON))),
                text_field: Some(json!(["get", "name"])),
                text_size: Some(Float(json!(TEXT))),
                text_anchor: Some(json!("top")),
                text_offset: Some(json!([0, 0.8])),
                ..Default::default()
            },
            paint: Some(Paint {
                icon_color: Some(Color(json!(palette.station))),
                icon_halo_color: Some(Color(json!(palette.casing))),
                icon_halo_width: Some(Float(json!(HALO_WIDTH))),
                text_color: Some(Color(json!(palette.station))),
                text_halo_color: Some(Color(json!(palette.casing))),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // tram_stops, marked with a tram, named under it
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "poi".into(),
            minzoom: Some(16.0),
            filter: Some(Filter(json!([
                "all",
                ["==", "class", "railway"],
                ["==", "subclass", "tram_stop"]
            ]))),
            layout: Layout {
                icon_image: Some(json!("tram")),
                icon_size: Some(Float(json!(SMALL_ICON))),
                text_field: Some(json!(["get", "name"])),
                text_size: Some(Float(json!(SMALL_TEXT))),
                text_anchor: Some(json!("top")),
                text_offset: Some(json!([0, 0.8])),
                ..Default::default()
            },
            paint: Some(Paint {
                icon_color: Some(Color(json!(palette.station))),
                icon_halo_color: Some(Color(json!(palette.casing))),
                icon_halo_width: Some(Float(json!(HALO_WIDTH))),
                text_color: Some(Color(json!(palette.station))),
                text_halo_color: Some(Color(json!(palette.casing))),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // bus_stops, marked with a bus, named under it
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "poi".into(),
            minzoom: Some(16.0),
            filter: Some(Filter(json!(["==", "class", "bus"]))),
            layout: Layout {
                icon_image: Some(json!("bus")),
                icon_size: Some(Float(json!(SMALL_ICON))),
                text_field: Some(json!(["get", "name"])),
                text_size: Some(Float(json!(SMALL_TEXT))),
                text_anchor: Some(json!("top")),
                text_offset: Some(json!([0, 0.8])),
                ..Default::default()
            },
            paint: Some(Paint {
                icon_color: Some(Color(json!(palette.station))),
                icon_halo_color: Some(Color(json!(palette.casing))),
                icon_halo_width: Some(Float(json!(HALO_WIDTH))),
                text_color: Some(Color(json!(palette.station))),
                text_halo_color: Some(Color(json!(palette.casing))),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
    ];

    Style { layers }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expression::Context;
    use egui::Color32;
    use serde_json::Value;
    use std::collections::HashMap;

    /// Size the style gives a label, found the way the renderer finds it: the layer which asks
    /// for this source layer and whose filter the feature passes.
    fn label_size(style: &Style, source: &str, properties: Value, zoom: u8) -> f32 {
        let context = Context::new(
            "Point".to_owned(),
            serde_json::from_value::<HashMap<String, Value>>(properties).unwrap(),
            zoom,
        );

        let sizes: Vec<f32> = style
            .layers
            .iter()
            .filter_map(|layer| match layer {
                StyleLayer::Symbol {
                    source_layer,
                    filter,
                    layout,
                    ..
                } if source_layer.matches(source)
                    && filter
                        .as_ref()
                        .is_some_and(|filter| filter.matches(&context)) =>
                {
                    layout
                        .text_size
                        .as_ref()
                        .map(|size| size.evaluate(&context))
                }
                _ => None,
            })
            .collect();

        assert_eq!(
            sizes.len(),
            1,
            "expected one label for {source}, got {sizes:?}"
        );
        sizes[0]
    }

    /// OpenMapTiles' `rank` is a placement priority where 1 is the most prominent — Berlin is 2,
    /// Wrocław 7, and the villages around it 11 to 15. Sizing labels by it drew villages larger
    /// than the city.
    #[test]
    fn a_city_is_labelled_larger_than_a_village() {
        let style = Style::openmaptiles_basemap_light();

        for zoom in [6, 10, 14] {
            assert!(
                label_size(&style, "place", json!({"class": "city", "rank": 7}), zoom)
                    > label_size(
                        &style,
                        "place",
                        json!({"class": "village", "rank": 12}),
                        zoom
                    ),
                "zoom {zoom}"
            );
        }
    }

    #[test]
    fn a_town_is_labelled_between_a_city_and_a_village() {
        let style = Style::openmaptiles_basemap_light();
        let size = |class| label_size(&style, "place", json!({ "class": class, "rank": 11 }), 12);

        assert!(size("city") > size("town"));
        assert!(size("town") > size("village"));
    }

    /// OpenMapTiles keeps house numbers in a layer of their own, only in its zoom 14 tiles. The
    /// label waits for the map, not the tile, to reach zoom 18.
    #[test]
    fn a_house_number_is_labelled_from_zoom_18() {
        let style = Style::openmaptiles_basemap_light();
        let context = Context::new(
            "Point".to_owned(),
            HashMap::from([("housenumber".to_owned(), json!("12a"))]),
            14,
        );

        let labels: Vec<_> = style
            .layers
            .iter()
            .filter_map(|layer| match layer {
                StyleLayer::Symbol {
                    source_layer,
                    filter,
                    layout,
                    minzoom,
                    ..
                } if source_layer.matches("housenumber")
                    && filter
                        .as_ref()
                        .is_none_or(|filter| filter.matches(&context)) =>
                {
                    Some((layout.text(&context), *minzoom))
                }
                _ => None,
            })
            .collect();

        assert_eq!(labels, [(Some("12a".to_owned()), Some(18.0))]);
    }

    /// Every class of road OpenMapTiles has a tunnel of.
    const ROAD_CLASSES: [&str; 9] = [
        "motorway",
        "trunk",
        "primary",
        "secondary",
        "tertiary",
        "minor",
        "service",
        "track",
        "path",
    ];

    /// Widths of the line layers which draw a feature of `source`, in the order they are drawn,
    /// found the way the renderer finds them.
    fn line_widths(style: &Style, source: &str, properties: Value, zoom: u8) -> Vec<f32> {
        let context = Context::new(
            "LineString".to_owned(),
            serde_json::from_value::<HashMap<String, Value>>(properties).unwrap(),
            zoom,
        );

        style
            .layers
            .iter()
            .filter_map(|layer| match layer {
                StyleLayer::Line {
                    source_layer,
                    filter,
                    paint,
                    ..
                } if source_layer.matches(source)
                    && filter
                        .as_ref()
                        .is_none_or(|filter| filter.matches(&context)) =>
                {
                    Some(
                        paint
                            .line_width
                            .as_ref()
                            .map_or(1.0, |width| width.evaluate(&context)),
                    )
                }
                _ => None,
            })
            .collect()
    }

    /// A tunnel is drawn as a dashed casing with the road over it, whatever the road. Motorway
    /// and trunk tunnels used to get only the casing.
    #[test]
    fn a_tunnel_has_a_casing_and_a_road() {
        let style = Style::openmaptiles_basemap_light();

        for class in ROAD_CLASSES {
            assert_eq!(
                line_widths(
                    &style,
                    "transportation",
                    json!({"class": class, "brunnel": "tunnel"}),
                    16
                )
                .len(),
                2,
                "{class}"
            );
        }
    }

    /// Each class of road grows, or at least does not shrink, as the map zooms in.
    #[test]
    fn a_road_never_gets_thinner_when_zooming_in() {
        let style = Style::openmaptiles_basemap_light();

        for class in ROAD_CLASSES {
            let widths: Vec<f32> = (0..=22)
                .map(|zoom| {
                    line_widths(&style, "transportation", json!({ "class": class }), zoom)
                        .into_iter()
                        .sum()
                })
                .collect();

            assert!(
                widths.windows(2).all(|pair| pair[0] <= pair[1]),
                "{class}: {widths:?}"
            );
        }
    }

    /// The casing is drawn under the road, so only what is wider than the road shows. It used to
    /// be a line a pixel wide, hidden as soon as the road grew past that.
    #[test]
    fn a_tunnel_casing_shows_either_side_of_its_road() {
        let style = Style::openmaptiles_basemap_light();

        for class in ROAD_CLASSES {
            for zoom in [15, 16, 18] {
                let widths = line_widths(
                    &style,
                    "transportation",
                    json!({"class": class, "brunnel": "tunnel"}),
                    zoom,
                );
                let [casing, road] = widths[..] else {
                    panic!("{class} at zoom {zoom}: expected a casing and a road, got {widths:?}");
                };

                assert!(
                    casing >= road + 1.0,
                    "{class} at zoom {zoom}: casing {casing}, road {road}"
                );
            }
        }
    }

    /// A ramp, such as a motorway exit, is drawn once, by its own road: as a casing and a road in
    /// a tunnel, and as one road elsewhere. A ramp layer of their own drew ramps a second time.
    #[test]
    fn a_ramp_is_drawn_once() {
        let style = Style::openmaptiles_basemap_light();
        let drawn = |properties| line_widths(&style, "transportation", properties, 16).len();

        for class in ["motorway", "trunk", "primary", "secondary"] {
            assert_eq!(drawn(json!({"class": class, "ramp": 1})), 1, "{class}");
            assert_eq!(
                drawn(json!({"class": class, "ramp": 1, "brunnel": "bridge"})),
                1,
                "{class} on a bridge"
            );
            assert_eq!(
                drawn(json!({"class": class, "ramp": 1, "brunnel": "tunnel"})),
                2,
                "{class} in a tunnel"
            );
        }
    }

    /// Texts which the style's label layers give a feature of `source`, at `zoom`, found the way
    /// the renderer finds them.
    fn labels(style: &Style, source: &str, properties: Value, zoom: u8) -> Vec<String> {
        let context = Context::new(
            "Point".to_owned(),
            serde_json::from_value::<HashMap<String, Value>>(properties).unwrap(),
            zoom,
        );

        style
            .layers
            .iter()
            .filter_map(|layer| match layer {
                StyleLayer::Symbol {
                    source_layer,
                    filter,
                    layout,
                    minzoom,
                    ..
                } if source_layer.matches(source)
                    && minzoom.is_none_or(|minzoom| f32::from(zoom) >= minzoom)
                    && filter
                        .as_ref()
                        .is_none_or(|filter| filter.matches(&context)) =>
                {
                    layout.text(&context)
                }
                _ => None,
            })
            .collect()
    }

    fn both_basemaps() -> [Style; 2] {
        [
            Style::openmaptiles_basemap_light(),
            Style::openmaptiles_basemap_dark(),
        ]
    }

    fn symbol_layouts(style: &Style) -> impl Iterator<Item = &Layout> {
        style.layers.iter().filter_map(|layer| match layer {
            StyleLayer::Symbol { layout, .. } => Some(layout),
            _ => None,
        })
    }

    #[test]
    fn every_icon_the_basemaps_name_is_built_in() {
        for style in both_basemaps() {
            let names: Vec<_> = symbol_layouts(&style)
                .filter_map(|layout| layout.icon_image.as_ref())
                .collect();

            assert!(!names.is_empty());
            for name in names {
                assert!(
                    name.as_str().and_then(crate::lookup_icon_outline).is_some(),
                    "no icon {name}"
                );
            }
        }
    }

    /// A symbol written as text, such as ⛰, is drawn only if the app's fonts have it, and egui's
    /// own did not. Symbols are icons instead.
    #[test]
    fn every_literal_text_of_the_basemaps_has_a_glyph_in_eguis_own_fonts() {
        let ctx = egui::Context::default();
        let font_id = egui::FontId::proportional(TEXT as f32);
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();

        for style in both_basemaps() {
            for text in
                symbol_layouts(&style).filter_map(|layout| layout.text_field.as_ref()?.as_str())
            {
                for c in text.chars() {
                    assert!(
                        ctx.fonts_mut(|fonts| fonts.has_glyph(&font_id, c)),
                        "{c:?} (U+{:04X}) of {text:?}",
                        c as u32
                    );
                }
            }
        }
    }

    /// What each layer marking a feature of `source_layer` draws for it: its icon and its text,
    /// and the colour of each.
    fn marks(
        style: &Style,
        source_layer: &str,
        properties: Value,
        zoom: u8,
    ) -> Vec<(Option<&'static str>, Option<String>, Color32, Color32)> {
        let context = Context::new(
            "Point".to_owned(),
            serde_json::from_value(properties).unwrap(),
            zoom,
        );

        style
            .layers
            .iter()
            .filter_map(|layer| match layer {
                StyleLayer::Symbol {
                    source_layer: layer_source_layer,
                    minzoom,
                    filter,
                    layout,
                    paint,
                    ..
                } if layer_source_layer.matches(source_layer)
                    && minzoom.is_none_or(|minzoom| f32::from(zoom) >= minzoom)
                    && filter
                        .as_ref()
                        .is_none_or(|filter| filter.matches(&context)) =>
                {
                    let paint = paint.as_ref()?;
                    Some((
                        layout.icon_image(&context).map(|outline| outline.name()),
                        layout.text(&context),
                        paint.icon_color.as_ref()?.evaluate(&context),
                        paint.text_color.as_ref()?.evaluate(&context),
                    ))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_peak_is_marked_with_a_mountain_in_the_colour_of_its_name() {
        let marks = marks(
            &Style::openmaptiles_basemap_light(),
            "mountain_peak",
            json!({"class": "peak", "name": "Ślęża", "ele": 718}),
            12,
        );

        let [(icon, text, icon_color, text_color)] = marks.as_slice() else {
            panic!("expected one layer marking the peak, got {marks:?}");
        };
        assert_eq!(*icon, Some("peak"));
        assert_eq!(text.as_deref(), Some("Ślęża\n718 m"));
        assert_eq!(icon_color, text_color);
    }

    #[test]
    fn an_airport_is_marked_with_a_plane_in_the_colour_of_its_name() {
        let marks = marks(
            &Style::openmaptiles_basemap_light(),
            "aerodrome_label",
            json!({"class": "international", "name": "Port Lotniczy Wrocław", "iata": "WRO"}),
            10,
        );

        let [(icon, text, icon_color, text_color)] = marks.as_slice() else {
            panic!("expected one layer marking the airport, got {marks:?}");
        };
        assert_eq!(*icon, Some("airport"));
        assert_eq!(text.as_deref(), Some("Port Lotniczy Wrocław"));
        assert_eq!(icon_color, text_color);
    }

    /// OpenMapTiles has every airfield, gliding clubs' and private strips included. Only the ones
    /// airlines fly to have an IATA code.
    #[test]
    fn an_airfield_without_an_iata_code_is_not_marked() {
        let marks = marks(
            &Style::openmaptiles_basemap_light(),
            "aerodrome_label",
            json!({"class": "private", "name": "Lotnisko Szymanów"}),
            12,
        );

        assert!(marks.is_empty(), "{marks:?}");
    }

    /// Before then, the airport would only crowd the names of the towns around it.
    #[test]
    fn an_airport_is_marked_from_zoom_10() {
        let airport =
            json!({"class": "international", "name": "Port Lotniczy Wrocław", "iata": "WRO"});
        let style = Style::openmaptiles_basemap_light();

        assert!(marks(&style, "aerodrome_label", airport.clone(), 9).is_empty());
        assert_eq!(marks(&style, "aerodrome_label", airport, 10).len(), 1);
    }

    #[test]
    fn a_hospital_is_marked_with_a_plus_in_the_colour_of_its_name() {
        let marks = marks(
            &Style::openmaptiles_basemap_light(),
            "poi",
            json!({"class": "hospital", "subclass": "hospital", "name": "Szpital Wojewódzki"}),
            14,
        );

        let [(icon, text, icon_color, text_color)] = marks.as_slice() else {
            panic!("expected one layer marking the hospital, got {marks:?}");
        };
        assert_eq!(*icon, Some("hospital"));
        assert_eq!(text.as_deref(), Some("Szpital Wojewódzki"));
        assert_eq!(icon_color, text_color);
    }

    /// OpenMapTiles puts clinics and nursing homes in the `hospital` class too, and one marked as
    /// a hospital would send someone there who needs one.
    #[test]
    fn a_clinic_or_nursing_home_is_not_marked_as_a_hospital() {
        let style = Style::openmaptiles_basemap_light();

        for subclass in ["clinic", "nursing_home"] {
            let marks = marks(
                &style,
                "poi",
                json!({"class": "hospital", "subclass": subclass, "name": "Przychodnia"}),
                16,
            );
            assert!(marks.is_empty(), "{subclass}: {marks:?}");
        }
    }

    #[test]
    fn a_hospital_is_marked_from_zoom_14() {
        let hospital = json!({"class": "hospital", "subclass": "hospital", "name": "Szpital"});
        let style = Style::openmaptiles_basemap_light();

        assert!(marks(&style, "poi", hospital.clone(), 13).is_empty());
        assert_eq!(marks(&style, "poi", hospital, 14).len(), 1);
    }

    /// OpenMapTiles has railway stations as a `railway` with a `station` subclass. The style
    /// asked for a `station` class, as Protomaps has it, and drew none.
    #[test]
    fn a_railway_station_is_named() {
        let style = Style::openmaptiles_basemap_light();
        let station = json!({"class": "railway", "subclass": "station", "name": "Wrocław Główny"});

        assert!(
            labels(&style, "poi", station, 14).contains(&"Wrocław Główny".to_owned()),
            "{:?}",
            labels(
                &style,
                "poi",
                json!({"class": "railway", "subclass": "station", "name": "Wrocław Główny"}),
                14
            )
        );
    }
}
