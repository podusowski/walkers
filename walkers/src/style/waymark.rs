//! Waymark, Walkers' style for OpenFreeMap, or anything else following the OpenMapTiles schema.
//!
//! <https://openmaptiles.org/schema/>

use super::{
    Color, Dasharray, Filter, Float, Layout, Paint, Style, StyleLayer, Value,
    exponential_zoom_interpolation, json, linear_zoom_interpolation,
};

/// What a style source of OpenMapTiles is called, as MapLibre styles usually call it.
const SOURCE: &str = "openmaptiles";

/// Waymark, Walkers' style for the OpenMapTiles schema, drawing from a source called
/// `openmaptiles`, and optionally relief and contour lines from sources of elevation.
///
/// ```
/// # use walkers::{Waymark, Palette};
/// let palette = Palette {
///     contour: "#006400".into(),
///     ..Palette::dark()
/// };
/// let style = Waymark::new(palette)
///     .hillshade("terrain")
///     .contours("terrain")
///     .build();
/// ```
#[derive(Clone, Debug)]
pub struct Waymark {
    palette: Palette,
    hillshade: Option<String>,
    contours: Option<String>,
}

impl Waymark {
    /// In the colors of `palette`.
    pub fn new(palette: Palette) -> Self {
        Self {
            palette,
            hillshade: None,
            contours: None,
        }
    }

    /// With the elevation of `source` shaded into relief, over land and water.
    pub fn hillshade(mut self, source: impl Into<String>) -> Self {
        self.hillshade = Some(source.into());
        self
    }

    /// With contour lines of the elevation of `source`, over land, water and relief. The major
    /// ones are thicker, and labelled with their elevation.
    pub fn contours(mut self, source: impl Into<String>) -> Self {
        self.contours = Some(source.into());
        self
    }

    pub fn build(self) -> Style {
        build(&self.palette, self.hillshade, self.contours)
    }
}

impl From<Waymark> for Style {
    fn from(waymark: Waymark) -> Self {
        waymark.build()
    }
}

impl Style {
    /// [`Waymark`] in its light palette.
    #[deprecated(note = "use `Waymark::new(Palette::light()).build()`")]
    pub fn openmaptiles_basemap_light() -> Self {
        Waymark::new(Palette::light()).build()
    }

    /// [`Waymark`] in its dark palette.
    #[deprecated(note = "use `Waymark::new(Palette::dark()).build()`")]
    pub fn openmaptiles_basemap_dark() -> Self {
        Waymark::new(Palette::dark()).build()
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

/// Colors of a [`Waymark`].
#[derive(Clone, Debug)]
pub struct Palette {
    /// Land, and the halo of road labels.
    pub background: Color,
    /// Railways.
    pub rail: Color,
    /// The dashes inside railways.
    pub rail_tie: Color,
    /// Tram lines.
    pub tram: Color,
    /// Parks, forests, farmland and other green land.
    pub forest: Color,
    /// Allotments, village greens and playgrounds.
    pub urban_green: Color,
    /// Piers.
    pub pier: Color,
    /// Outlines of tunnels.
    pub tunnel_casing: Color,
    /// The halo of place, stop and peak labels, and of house numbers.
    pub casing: Color,
    /// Hospitals, zoos, airports and pedestrian areas.
    pub landuse_dark: Color,
    /// Beaches.
    pub bridge: Color,
    /// Tunnels and buildings.
    pub structure: Color,
    /// Paths and tracks.
    pub path: Color,
    /// Cycleways.
    pub cycleway: Color,
    /// Runways and taxiways.
    pub muted: Color,
    /// Motorways.
    pub highway: Color,
    /// Other roads.
    pub road: Color,
    /// Labels of roads and places, house numbers, and country borders.
    pub label: Color,
    /// Labels of cities, towns and villages.
    pub locality_text: Color,
    /// Water.
    pub water: Color,
    /// Labels of water.
    pub water_label: Color,
    /// The halo of water labels.
    pub water_label_halo: Color,
    /// Stations and stops, and their labels.
    pub station: Color,
    /// Peaks, and their labels.
    pub peak: Color,
    /// Shadows of the relief.
    pub hillshade_shadow: Color,
    /// Highlights of the relief.
    pub hillshade_highlight: Color,
    /// Accents of the relief, on its steepest slopes.
    pub hillshade_accent: Color,
    /// Contour lines, the minor ones more transparent than the major ones.
    pub contour: Color,
    /// Labels of contour lines.
    pub contour_label: Color,
    /// The halo of contour labels.
    pub contour_halo: Color,
}

impl Palette {
    /// Light on dark.
    pub fn dark() -> Self {
        Self {
            background: "#000000".into(),
            rail_tie: "#6d6d6d".into(),
            tram: "#2d2d2d".into(),
            rail: "#000000".into(),
            forest: "#061009".into(),
            urban_green: "#000c00".into(),
            pier: "#0a0a0a".into(),
            tunnel_casing: "#101010".into(),
            casing: "#141414".into(),
            landuse_dark: "#191919".into(),
            bridge: "#1f1f1f".into(),
            structure: "#292929".into(),
            path: "#5a4a3a".into(),
            cycleway: "#74463a".into(),
            muted: "#333333".into(),
            highway: "#352121".into(),
            road: "#464646".into(),
            label: "#a0a0a0".into(),
            locality_text: "#999999".into(),
            water: "#161e31".into(),
            water_label: "#134164".into(),
            water_label_halo: "#0d1526".into(),
            station: "#98b07a".into(),
            peak: "#b59a6a".into(),
            hillshade_shadow: "#000000".into(),
            hillshade_highlight: "#ffffff".into(),
            hillshade_accent: "#000000".into(),
            contour: "#334433".into(),
            contour_label: "rgba(51, 68, 51, 0.8)".into(),
            contour_halo: "rgba(0, 0, 0, 0.6)".into(),
        }
    }

    /// Dark on light.
    pub fn light() -> Self {
        Self {
            background: "#f2f2f0".into(),
            rail_tie: "#000000".into(),
            tram: "#000000".into(),
            rail: "#ffffff".into(),
            forest: "#86a180".into(),
            urban_green: "#e2f0df".into(),
            pier: "#8c8c8c".into(),
            tunnel_casing: "#595959".into(),
            casing: "#e9e9e9".into(),
            landuse_dark: "#e0e0e0".into(),
            bridge: "#bfbfbf".into(),
            structure: "#5b5b5b".into(),
            path: "#4c3316".into(),
            cycleway: "#b86b6b".into(),
            muted: "#b3b3b3".into(),
            highway: "#bb5f5f".into(),
            road: "#4a4a4a".into(),
            label: "#000000".into(),
            locality_text: "#1a1a1a".into(),
            water: "#88b2e2".into(),
            water_label: "#2f6690".into(),
            water_label_halo: "#ffffff".into(),
            station: "#3f5934".into(),
            peak: "#6b5330".into(),
            hillshade_shadow: "#000000".into(),
            hillshade_highlight: "#ffffff".into(),
            hillshade_accent: "#000000".into(),
            contour: "#334433".into(),
            contour_label: "rgba(51, 68, 51, 0.8)".into(),
            contour_halo: "rgba(255, 255, 255, 0.6)".into(),
        }
    }
}

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

const MINOR_CONTOUR_WIDTH: f64 = 0.6;
const MAJOR_CONTOUR_WIDTH: f64 = 1.2;
const MINOR_CONTOUR_OPACITY: f64 = 0.3;
const MAJOR_CONTOUR_OPACITY: f64 = 0.5;

const SMALL_TEXT: f64 = 10.0;
const TEXT: f64 = 12.0;
const LARGE_TEXT: f64 = 14.0;

fn road_width(factor: f64) -> Float {
    exponential_zoom_interpolation(1.6, &ROAD_WIDTH.map(|(zoom, width)| (zoom, width * factor)))
}

/// Width of a casing: its line, with a pixel either side. That is never wider than the line, so
/// that a line too thin to see does not get a casing which is not.
fn casing_width(line: Value) -> Float {
    Float(json!(["+", line, ["*", 2, ["min", 1, line]]]))
}

/// Dash and gap of an unpaved road, in widths of the road.
const UNPAVED_ROAD_DASHES: [f64; 2] = [2.0, 1.0];

/// Dashes for an unpaved road, and none for a paved road or one of unknown surface.
fn surface_dashes() -> Dasharray {
    Dasharray(json!([
        "match",
        ["get", "surface"],
        "unpaved",
        ["literal", UNPAVED_ROAD_DASHES],
        ["literal", []]
    ]))
}

/// Paths and tracks, dashed in `color`, and cycleways, solid in `cycleway_color`.
fn path_paint(color: &Color, cycleway_color: &Color) -> Paint {
    Paint {
        line_color: Some(Color(json!([
            "match",
            ["get", "subclass"],
            "cycleway",
            cycleway_color.0,
            color.0
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

/// Relief shaded from `hillshade`, and contour lines traced from `contours`, under them the minor
/// ones.
fn relief(
    palette: &Palette,
    hillshade: Option<String>,
    contours: Option<String>,
) -> Vec<StyleLayer> {
    let hillshade = hillshade.map(|source| StyleLayer::Hillshade {
        source: Some(source),
        paint: Paint {
            hillshade_shadow_color: Some(palette.hillshade_shadow.clone()),
            hillshade_highlight_color: Some(palette.hillshade_highlight.clone()),
            hillshade_accent_color: Some(palette.hillshade_accent.clone()),
            ..Default::default()
        },
    });

    let contours = contours.into_iter().flat_map(|source| {
        let contour = |level: i32, opacity: f64, layout, paint: Paint| StyleLayer::Contour {
            source: Some(source.clone()),
            filter: Some(Filter(json!(["==", ["get", "level"], level]))),
            layout,
            paint: Some(Paint {
                line_color: Some(palette.contour.clone()),
                line_opacity: Some(Float(json!(opacity))),
                ..paint
            }),
        };

        [
            contour(
                0,
                MINOR_CONTOUR_OPACITY,
                Layout::default(),
                Paint {
                    line_width: Some(Float(json!(MINOR_CONTOUR_WIDTH))),
                    ..Default::default()
                },
            ),
            contour(
                1,
                MAJOR_CONTOUR_OPACITY,
                Layout {
                    text_field: Some(json!(["to-string", ["get", "ele"]])),
                    text_size: Some(Float(json!(TEXT))),
                    ..Default::default()
                },
                Paint {
                    line_width: Some(Float(json!(MAJOR_CONTOUR_WIDTH))),
                    text_color: Some(palette.contour_label.clone()),
                    text_halo_color: Some(palette.contour_halo.clone()),
                    text_halo_width: Some(Float(json!(HALO_WIDTH))),
                    ..Default::default()
                },
            ),
        ]
    });

    hillshade.into_iter().chain(contours).collect()
}

fn build(palette: &Palette, hillshade: Option<String>, contours: Option<String>) -> Style {
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

    let mut layers = vec![
        // background
        StyleLayer::Background {
            source: Some(SOURCE.into()),
            paint: Paint {
                background_color: Some(palette.background.clone()),
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
                fill_color: Some(palette.forest.clone()),
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
                fill_color: Some(palette.urban_green.clone()),
                ..Default::default()
            },
        },
        // landuse_hospital
        StyleLayer::Fill {
            source: Some(SOURCE.into()),
            source_layer: LANDUSE.into(),
            filter: Some(Filter(json!(["==", "class", "hospital"]))),
            paint: Paint {
                fill_color: Some(palette.landuse_dark.clone()),
                ..Default::default()
            },
        },
        // landuse_beach
        StyleLayer::Fill {
            source: Some(SOURCE.into()),
            source_layer: LANDUSE.into(),
            filter: Some(Filter(json!(["in", "class", "beach"]))),
            paint: Paint {
                fill_color: Some(palette.bridge.clone()),
                ..Default::default()
            },
        },
        // landuse_zoo
        StyleLayer::Fill {
            source: Some(SOURCE.into()),
            source_layer: LANDUSE.into(),
            filter: Some(Filter(json!(["in", "class", "zoo"]))),
            paint: Paint {
                fill_color: Some(palette.landuse_dark.clone()),
                ..Default::default()
            },
        },
        // landuse_aerodrome
        StyleLayer::Fill {
            source: Some(SOURCE.into()),
            source_layer: LANDUSE.into(),
            filter: Some(Filter(json!(["in", "class", "aerodrome"]))),
            paint: Paint {
                fill_color: Some(palette.landuse_dark.clone()),
                ..Default::default()
            },
        },
        // roads_runway
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "transportation".into(),
            filter: Some(Filter(json!(["==", "subclass", "runway"]))),
            paint: Paint {
                line_color: Some(palette.muted.clone()),
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
                line_color: Some(palette.muted.clone()),
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
                fill_color: Some(palette.muted.clone()),
                ..Default::default()
            },
        },
        // water
        StyleLayer::Fill {
            source: Some(SOURCE.into()),
            source_layer: "water".into(),
            filter: Some(Filter(json!(["==", ["geometry-type"], "Polygon"]))),
            paint: Paint {
                fill_color: Some(palette.water.clone()),
                ..Default::default()
            },
        },
        // water_stream, and canals, which are wider
        StyleLayer::Line {
            source: Some(SOURCE.into()),
            source_layer: "waterway".into(),
            filter: Some(Filter(json!(["in", "class", "stream", "canal"]))),
            paint: Paint {
                line_color: Some(palette.water.clone()),
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
                line_color: Some(palette.water.clone()),
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
                fill_color: Some(palette.landuse_dark.clone()),
                ..Default::default()
            },
        },
        // landuse_pier
        StyleLayer::Fill {
            source: Some(SOURCE.into()),
            source_layer: LANDUSE.into(),
            filter: Some(Filter(json!(["==", "class", "pier"]))),
            paint: Paint {
                fill_color: Some(palette.pier.clone()),
                ..Default::default()
            },
        },
    ];

    // Over land and water, under everything else.
    layers.extend(relief(palette, hillshade, contours));

    layers.extend([
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
                line_color: Some(palette.tunnel_casing.clone()),
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
                line_color: Some(palette.tunnel_casing.clone()),
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
                line_color: Some(palette.tunnel_casing.clone()),
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
                line_color: Some(palette.tunnel_casing.clone()),
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
            paint: path_paint(&palette.structure, &palette.structure),
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
                line_color: Some(palette.structure.clone()),
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
                line_color: Some(palette.structure.clone()),
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
                line_color: Some(palette.structure.clone()),
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
                fill_color: Some(palette.structure.clone()),
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
                line_color: Some(palette.pier.clone()),
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
            paint: path_paint(&palette.path, &palette.cycleway),
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
                line_color: Some(palette.road.clone()),
                line_width: Some(Float(minor_road_width.clone())),
                line_dasharray: Some(surface_dashes()),
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
                line_color: Some(palette.road.clone()),
                line_width: Some(Float(major_road_width.clone())),
                line_dasharray: Some(surface_dashes()),
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
                line_color: Some(palette.highway.clone()),
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
                line_color: Some(palette.rail.clone()),
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
                line_color: Some(palette.rail_tie.clone()),
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
                line_color: Some(palette.tram.clone()),
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
                line_color: Some(palette.label.clone()),
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
            paint: path_paint(&palette.path, &palette.cycleway),
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
                line_color: Some(palette.road.clone()),
                line_width: Some(Float(minor_road_width.clone())),
                line_dasharray: Some(surface_dashes()),
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
                line_color: Some(palette.road.clone()),
                line_width: Some(Float(major_road_width.clone())),
                line_dasharray: Some(surface_dashes()),
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
                line_color: Some(palette.highway.clone()),
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
                text_color: Some(palette.label.clone()),
                text_halo_color: Some(palette.casing.clone()),
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
                text_color: Some(palette.water_label.clone()),
                text_halo_color: Some(palette.water_label_halo.clone()),
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
                text_color: Some(palette.label.clone()),
                text_halo_color: Some(palette.background.clone()),
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
                text_color: Some(palette.water_label.clone()),
                text_halo_color: Some(palette.water_label_halo.clone()),
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
                text_color: Some(palette.water_label.clone()),
                text_halo_color: Some(palette.water_label_halo.clone()),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // roads_labels_junction_symbol, motorway exits
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "transportation_name".into(),
            minzoom: None,
            filter: Some(Filter(json!(["==", "subclass", "junction"]))),
            layout: Layout {
                text_field: Some(json!("⬈")),
                text_size: Some(Float(json!(TEXT))),
                ..Default::default()
            },
            paint: Some(Paint {
                text_color: Some(palette.label.clone()),
                text_halo_color: Some(palette.background.clone()),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // roads_labels_junction, named under the symbol
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "transportation_name".into(),
            minzoom: None,
            filter: Some(Filter(json!(["==", "subclass", "junction"]))),
            layout: Layout {
                text_field: Some(json!(["get", "name"])),
                text_size: Some(Float(json!(SMALL_TEXT))),
                text_anchor: Some(json!("top")),
                text_offset: Some(json!([0, 0.8])),
                ..Default::default()
            },
            paint: Some(Paint {
                text_color: Some(palette.label.clone()),
                text_halo_color: Some(palette.background.clone()),
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
                text_color: Some(palette.label.clone()),
                text_halo_color: Some(palette.background.clone()),
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
                text_color: Some(palette.label.clone()),
                text_halo_color: Some(palette.casing.clone()),
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
                text_color: Some(palette.label.clone()),
                text_halo_color: Some(palette.casing.clone()),
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
                text_color: Some(palette.locality_text.clone()),
                text_halo_color: Some(palette.casing.clone()),
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
                text_color: Some(palette.label.clone()),
                text_halo_color: Some(palette.casing.clone()),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // stations_symbol
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
                text_field: Some(json!("🚆")),
                text_size: Some(Float(json!(TEXT))),
                ..Default::default()
            },
            paint: Some(Paint {
                text_color: Some(palette.station.clone()),
                text_halo_color: Some(palette.casing.clone()),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // stations, named under the symbol
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
                text_field: Some(json!(["get", "name"])),
                text_size: Some(Float(json!(TEXT))),
                text_anchor: Some(json!("top")),
                text_offset: Some(json!([0, 0.8])),
                ..Default::default()
            },
            paint: Some(Paint {
                text_color: Some(palette.station.clone()),
                text_halo_color: Some(palette.casing.clone()),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // tram_stops_symbol
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
                text_field: Some(json!("🚊")),
                text_size: Some(Float(json!(SMALL_TEXT))),
                ..Default::default()
            },
            paint: Some(Paint {
                text_color: Some(palette.station.clone()),
                text_halo_color: Some(palette.casing.clone()),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // tram_stops, named under the symbol
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
                text_field: Some(json!(["get", "name"])),
                text_size: Some(Float(json!(SMALL_TEXT))),
                text_anchor: Some(json!("top")),
                text_offset: Some(json!([0, 0.8])),
                ..Default::default()
            },
            paint: Some(Paint {
                text_color: Some(palette.station.clone()),
                text_halo_color: Some(palette.casing.clone()),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // bus_stops_symbol
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "poi".into(),
            minzoom: Some(16.0),
            filter: Some(Filter(json!(["==", "class", "bus"]))),
            layout: Layout {
                text_field: Some(json!("🚌")),
                text_size: Some(Float(json!(SMALL_TEXT))),
                ..Default::default()
            },
            paint: Some(Paint {
                text_color: Some(palette.station.clone()),
                text_halo_color: Some(palette.casing.clone()),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // bus_stops, named under the symbol
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "poi".into(),
            minzoom: Some(16.0),
            filter: Some(Filter(json!(["==", "class", "bus"]))),
            layout: Layout {
                text_field: Some(json!(["get", "name"])),
                text_size: Some(Float(json!(SMALL_TEXT))),
                text_anchor: Some(json!("top")),
                text_offset: Some(json!([0, 0.8])),
                ..Default::default()
            },
            paint: Some(Paint {
                text_color: Some(palette.station.clone()),
                text_halo_color: Some(palette.casing.clone()),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // peaks_symbol, on the peak itself
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "mountain_peak".into(),
            minzoom: None,
            filter: Some(Filter(json!(["in", "class", "peak", "volcano"]))),
            layout: Layout {
                text_field: Some(json!("⛰")),
                text_size: Some(Float(json!(TEXT))),
                ..Default::default()
            },
            paint: Some(Paint {
                text_color: Some(palette.peak.clone()),
                text_halo_color: Some(palette.casing.clone()),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
        // peaks, named under the symbol
        StyleLayer::Symbol {
            source: Some(SOURCE.into()),
            source_layer: "mountain_peak".into(),
            minzoom: None,
            filter: Some(Filter(json!(["in", "class", "peak", "volcano"]))),
            layout: Layout {
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
                text_color: Some(palette.peak.clone()),
                text_halo_color: Some(palette.casing.clone()),
                text_halo_width: Some(Float(json!(HALO_WIDTH))),
                ..Default::default()
            }),
        },
    ]);

    Style { layers }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expression::Context;
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
        let style = Waymark::new(Palette::light()).build();

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
        let style = Waymark::new(Palette::light()).build();
        let size = |class| label_size(&style, "place", json!({ "class": class, "rank": 11 }), 12);

        assert!(size("city") > size("town"));
        assert!(size("town") > size("village"));
    }

    /// OpenMapTiles keeps house numbers in a layer of their own, only in its zoom 14 tiles. The
    /// label waits for the map, not the tile, to reach zoom 18.
    #[test]
    fn a_house_number_is_labelled_from_zoom_18() {
        let style = Waymark::new(Palette::light()).build();
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
        let style = Waymark::new(Palette::light()).build();

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
        let style = Waymark::new(Palette::light()).build();

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
        let style = Waymark::new(Palette::light()).build();

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

    /// Dash patterns of the line layers which draw a feature of `source`, in the order they are
    /// drawn, empty for a solid line. Found the way the renderer finds them.
    fn dashes(style: &Style, source: &str, properties: Value, zoom: u8) -> Vec<Vec<f32>> {
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
                            .line_dasharray
                            .as_ref()
                            .and_then(|dashes| dashes.evaluate(&context))
                            .unwrap_or_default(),
                    )
                }
                _ => None,
            })
            .collect()
    }

    /// An unpaved road is dashed, on a bridge too, and a paved one, or one of unknown surface,
    /// is solid.
    #[test]
    fn an_unpaved_road_is_dashed() {
        let style = Waymark::new(Palette::light()).build();

        for class in ["primary", "secondary", "tertiary", "minor", "service"] {
            for brunnel in [None, Some("bridge")] {
                let road = |surface: Option<&str>| {
                    let mut properties = json!({ "class": class });
                    if let Some(surface) = surface {
                        properties["surface"] = json!(surface);
                    }
                    if let Some(brunnel) = brunnel {
                        properties["brunnel"] = json!(brunnel);
                    }
                    dashes(&style, "transportation", properties, 16)
                };

                let [unpaved] = &road(Some("unpaved"))[..] else {
                    panic!("{class} {brunnel:?}: expected one line");
                };
                assert!(!unpaved.is_empty(), "{class} {brunnel:?}");
                assert_eq!(
                    road(Some("paved")),
                    [Vec::<f32>::new()],
                    "{class} {brunnel:?}"
                );
                assert_eq!(road(None), [Vec::<f32>::new()], "{class} {brunnel:?}");
            }
        }
    }

    /// A ramp, such as a motorway exit, is drawn once, by its own road: as a casing and a road in
    /// a tunnel, and as one road elsewhere. A ramp layer of their own drew ramps a second time.
    #[test]
    fn a_ramp_is_drawn_once() {
        let style = Waymark::new(Palette::light()).build();
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

    /// OpenMapTiles has railway stations as a `railway` with a `station` subclass. The style
    /// asked for a `station` class, as Protomaps has it, and drew none.
    #[test]
    fn a_railway_station_is_named() {
        let style = Waymark::new(Palette::light()).build();
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

    fn relief() -> Style {
        Waymark::new(Palette::light())
            .hillshade("terrain")
            .contours("contours")
            .build()
    }

    #[test]
    fn relief_and_contour_lines_are_over_land_and_water_and_under_roads() {
        let style = relief();
        let hillshade = style
            .layers
            .iter()
            .position(|layer| matches!(layer, StyleLayer::Hillshade { .. }))
            .unwrap();
        let last_ground = style
            .layers
            .iter()
            .rposition(|layer| {
                matches!(layer, StyleLayer::Fill { source_layer, .. }
                    if source_layer.matches("landuse") || source_layer.matches("water"))
            })
            .unwrap();
        assert_eq!(hillshade, last_ground + 1);
        assert!(matches!(
            &style.layers[hillshade + 1..hillshade + 3],
            [StyleLayer::Contour { .. }, StyleLayer::Contour { .. }]
        ));
        assert!(matches!(
            &style.layers[hillshade + 3],
            StyleLayer::Line { source_layer, .. } if source_layer.matches("transportation")
        ));
    }

    #[test]
    fn relief_and_contour_lines_draw_from_their_sources() {
        let style = relief();
        let sources: Vec<_> = style
            .layer_sources()
            .filter_map(|(source, drawn)| drawn.and(source))
            .collect();
        assert_eq!(sources, ["terrain", "contours", "contours"]);
    }

    #[test]
    fn there_is_no_relief_unless_asked_for() {
        assert!(
            Waymark::new(Palette::dark())
                .build()
                .layer_sources()
                .all(|(_, drawn)| drawn.is_none())
        );
    }

    #[test]
    fn contour_lines_are_of_the_palette_color() {
        let style = Waymark::new(Palette {
            contour: "#c80000".into(),
            ..Palette::dark()
        })
        .contours("terrain")
        .build();
        let context = Context::new("LineString".to_owned(), HashMap::new(), 12);

        let lines: Vec<_> = style
            .layers
            .iter()
            .filter_map(|layer| match layer {
                StyleLayer::Contour {
                    paint: Some(paint), ..
                } => Some((
                    paint.line_color.as_ref()?.evaluate(&context),
                    paint.line_opacity.as_ref()?.evaluate(&context),
                )),
                _ => None,
            })
            .collect();

        let red = egui::Color32::from_rgb(200, 0, 0);
        assert_eq!(lines, [(red, 0.3), (red, 0.5)]);
    }
}
