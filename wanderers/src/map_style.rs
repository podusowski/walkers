use walkers::{
    Color, Dasharray, Filter, Float, Layer, Layout, Paint, SourceLayer, Style, Value, json,
};

/// Which vector tile schema the tiles follow.
///
/// Only the names live here. Differing property values are absorbed by the filters, which is
/// not possible for layer names because `source_layer` is read before there is any feature.
#[derive(Clone, Copy)]
pub struct Schema {
    earth: &'static str,
    landuse: &'static [&'static str],
    water: &'static str,
    waterway: &'static str,
    roads: &'static str,
    road_labels: &'static str,
    buildings: &'static str,
    places: &'static str,
    pois: &'static str,
    boundaries: &'static str,
    kind: &'static str,
    kind_detail: &'static str,
    place_rank: &'static str,
    brunnel: Option<&'static str>,
    link: &'static str,
}

pub const PROTOMAPS: Schema = Schema {
    earth: "earth",
    landuse: &["landuse"],
    water: "water",
    waterway: "water",
    roads: "roads",
    road_labels: "roads",
    buildings: "buildings",
    places: "places",
    pois: "pois",
    boundaries: "boundaries",
    kind: "kind",
    kind_detail: "kind_detail",
    place_rank: "population_rank",
    brunnel: None,
    link: "is_link",
};

/// `earth` is empty because there is no land polygon here; the background stands in for it.
pub const OPENMAPTILES: Schema = Schema {
    earth: "",
    landuse: &["landcover", "landuse", "park"],
    water: "water",
    waterway: "waterway",
    roads: "transportation",
    road_labels: "transportation_name",
    buildings: "building",
    places: "place",
    pois: "poi",
    boundaries: "boundary",
    kind: "class",
    kind_detail: "subclass",
    place_rank: "rank",
    brunnel: Some("brunnel"),
    link: "ramp",
};

/// A road which bridges or tunnels rather than lying on the ground.
#[derive(Clone, Copy)]
pub enum Brunnel {
    Tunnel,
    Bridge,
}

impl Brunnel {
    fn protomaps(self) -> &'static str {
        match self {
            Brunnel::Tunnel => "is_tunnel",
            Brunnel::Bridge => "is_bridge",
        }
    }

    fn openmaptiles(self) -> &'static str {
        match self {
            Brunnel::Tunnel => "tunnel",
            Brunnel::Bridge => "bridge",
        }
    }
}

impl Schema {
    fn is(&self, what: Brunnel) -> Value {
        match self.brunnel {
            Some(key) => json!(["==", key, what.openmaptiles()]),
            None => json!(["has", what.protomaps()]),
        }
    }

    fn is_not(&self, what: Brunnel) -> Value {
        match self.brunnel {
            Some(key) => json!(["!=", key, what.openmaptiles()]),
            None => json!(["!has", what.protomaps()]),
        }
    }

    fn is_link(&self) -> Value {
        match self.brunnel {
            Some(_) => json!(["==", self.link, 1]),
            None => json!(["has", self.link]),
        }
    }

    fn is_not_link(&self) -> Value {
        match self.brunnel {
            Some(_) => json!(["!=", self.link, 1]),
            None => json!(["!has", self.link]),
        }
    }
}

fn linear_zoom_interpolation(stops: &[(f64, f64)]) -> Float {
    let mut expr = vec![json!("interpolate"), json!(["linear"]), json!(["zoom"])];
    for &(zoom, value) in stops {
        expr.push(json!(zoom));
        expr.push(json!(value));
    }
    Float(json!(expr))
}

struct Palette {
    background: &'static str,
    rail: &'static str,
    rail_tie: &'static str,
    forest: &'static str,
    urban_green: &'static str,
    pier: &'static str,
    tunnel_casing: &'static str,
    casing: &'static str,
    landuse_dark: &'static str,
    bridge: &'static str,
    structure: &'static str,
    muted: &'static str,
    highway: &'static str,
    road: &'static str,
    label_muted: &'static str,
    major_road_border: &'static str,
    label: &'static str,
    locality_text: &'static str,
    water: &'static str,
    station: &'static str,
}

const DARK: Palette = Palette {
    background: "#000000",
    rail_tie: "#bfbfbf",
    rail: "#000000",
    forest: "#061009",
    urban_green: "#000c00",
    pier: "#0a0a0a",
    tunnel_casing: "#101010",
    casing: "#141414",
    landuse_dark: "#191919",
    bridge: "#1f1f1f",
    structure: "#292929",
    muted: "#333333",
    highway: "#352121",
    road: "#464646",
    label_muted: "#5c5c5c",
    major_road_border: "#696868",
    label: "#707070",
    locality_text: "#999999",
    water: "#161e31",
    station: "#7fb3d9",
};

const LIGHT: Palette = Palette {
    background: "#f2f2f0",
    rail_tie: "#000000",
    //rail_tie: "#cccccc",
    rail: "#ffffff",
    forest: "#86a180",
    urban_green: "#e2f0df",
    pier: "#8c8c8c",
    tunnel_casing: "#595959",
    casing: "#ffffff",
    landuse_dark: "#e0e0e0",
    bridge: "#bfbfbf",
    structure: "#d4d4d4",
    muted: "#b3b3b3",
    highway: "#bb5f5f",
    road: "#4a4a4a",
    label_muted: "#595959",
    major_road_border: "#1e1e1e",
    label: "#1f1f1f",
    locality_text: "#1a1a1a",
    water: "#88b2e2",
    station: "#2b6ca3",
};

fn build(palette: &Palette, schema: Schema) -> Style {
    let mut layers = vec![
        // background
        Layer::Background {
            paint: Paint {
                background_color: Some(Color(json!(palette.background))),
                ..Default::default()
            },
        },
        // earth
        Layer::Fill {
            source_layer: schema.earth.into(),
            filter: Some(Filter(json!(["==", "$type", "Polygon"]))),
            paint: Paint {
                fill_color: Some(Color(json!(palette.background))),
                ..Default::default()
            },
        },
        // landuse_park
        Layer::Fill {
            source_layer: schema.landuse.into(),
            filter: Some(Filter(json!([
                "in",
                schema.kind,
                "national_park",
                "park",
                "public_park",
                "nature_reserve",
                "cemetery",
                "nature_reserve",
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
        Layer::Fill {
            source_layer: schema.landuse.into(),
            filter: Some(Filter(json!([
                "in",
                schema.kind,
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
        Layer::Fill {
            source_layer: schema.landuse.into(),
            filter: Some(Filter(json!(["==", schema.kind, "hospital"]))),
            paint: Paint {
                fill_color: Some(Color(json!(palette.landuse_dark))),
                ..Default::default()
            },
        },
        // landuse_industrial
        //Layer::Fill {
        //    source_layer: schema.landuse.into(),
        //    filter: Some(Filter(json!(["==", schema.kind, "industrial"]))),
        //    paint: Paint {
        //        fill_color: Some(Color(json!(palette.tunnel_casing))),
        //        ..Default::default()
        //    },
        //},
        // landuse_beach
        Layer::Fill {
            source_layer: schema.landuse.into(),
            filter: Some(Filter(json!(["in", schema.kind, "beach"]))),
            paint: Paint {
                fill_color: Some(Color(json!(palette.bridge))),
                ..Default::default()
            },
        },
        // landuse_zoo
        Layer::Fill {
            source_layer: schema.landuse.into(),
            filter: Some(Filter(json!(["in", schema.kind, "zoo"]))),
            paint: Paint {
                fill_color: Some(Color(json!(palette.landuse_dark))),
                ..Default::default()
            },
        },
        // landuse_aerodrome
        Layer::Fill {
            source_layer: schema.landuse.into(),
            filter: Some(Filter(json!(["in", schema.kind, "aerodrome"]))),
            paint: Paint {
                fill_color: Some(Color(json!(palette.landuse_dark))),
                ..Default::default()
            },
        },
        // roads_runway
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!(["==", schema.kind_detail, "runway"]))),
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
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!(["==", schema.kind_detail, "taxiway"]))),
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
        Layer::Fill {
            source_layer: schema.landuse.into(),
            filter: Some(Filter(json!(["in", schema.kind, "runway", "taxiway"]))),
            paint: Paint {
                fill_color: Some(Color(json!(palette.muted))),
                ..Default::default()
            },
        },
        // water
        Layer::Fill {
            source_layer: schema.water.into(),
            filter: Some(Filter(json!(["==", "$type", "Polygon"]))),
            paint: Paint {
                fill_color: Some(Color(json!(palette.water))),
                ..Default::default()
            },
        },
        // water_stream
        Layer::Line {
            source_layer: schema.waterway.into(),
            filter: Some(Filter(json!(["in", schema.kind, "stream"]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.water))),
                ..Default::default()
            },
        },
        // water_river
        Layer::Line {
            source_layer: schema.waterway.into(),
            filter: Some(Filter(json!(["in", schema.kind, "river"]))),
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
        Layer::Fill {
            source_layer: schema.landuse.into(),
            filter: Some(Filter(json!(["in", schema.kind, "pedestrian", "dam"]))),
            paint: Paint {
                fill_color: Some(Color(json!(palette.landuse_dark))),
                ..Default::default()
            },
        },
        // landuse_pier
        Layer::Fill {
            source_layer: schema.landuse.into(),
            filter: Some(Filter(json!(["==", schema.kind, "pier"]))),
            paint: Paint {
                fill_color: Some(Color(json!(palette.pier))),
                ..Default::default()
            },
        },
        // roads_tunnels_other_casing
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is(Brunnel::Tunnel),
                ["in", schema.kind, "other", "path", "service", "track"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.tunnel_casing))),
                ..Default::default()
            },
        },
        // roads_tunnels_minor_casing
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is(Brunnel::Tunnel),
                ["in", schema.kind, "minor_road", "minor", "tertiary"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.tunnel_casing))),
                line_dasharray: Some(Dasharray(json!([3, 2]))),
                line_width: Some(linear_zoom_interpolation(&[(12.0, 0.0), (12.5, 1.0)])),
                ..Default::default()
            },
        },
        // roads_tunnels_link_casing
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is(Brunnel::Tunnel),
                schema.is_link()
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.tunnel_casing))),
                line_dasharray: Some(Dasharray(json!([3, 2]))),
                line_width: Some(linear_zoom_interpolation(&[(12.0, 0.0), (12.5, 1.0)])),
                ..Default::default()
            },
        },
        // roads_tunnels_major_casing
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is_not(Brunnel::Tunnel),
                schema.is_not(Brunnel::Bridge),
                ["in", schema.kind, "major_road", "primary", "secondary"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.tunnel_casing))),
                line_dasharray: Some(Dasharray(json!([3, 2]))),
                line_width: Some(linear_zoom_interpolation(&[(9.0, 0.0), (9.5, 1.0)])),
                ..Default::default()
            },
        },
        // roads_tunnels_highway_casing
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is_not(Brunnel::Tunnel),
                schema.is_not(Brunnel::Bridge),
                ["in", schema.kind, "highway", "motorway", "trunk"],
                schema.is_not_link()
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.tunnel_casing))),
                line_dasharray: Some(Dasharray(json!([6, 0.5]))),
                line_width: Some(linear_zoom_interpolation(&[
                    (7.0, 0.0),
                    (7.5, 1.5),
                    (20.0, 22.5),
                ])),
                ..Default::default()
            },
        },
        // roads_tunnels_other
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is(Brunnel::Tunnel),
                ["in", schema.kind, "other", "path", "service", "track"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.structure))),
                line_dasharray: Some(Dasharray(json!([4.5, 0.5]))),
                line_width: Some(linear_zoom_interpolation(&[(14.0, 0.0), (20.0, 7.0)])),
                ..Default::default()
            },
        },
        // roads_tunnels_minor
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is(Brunnel::Tunnel),
                ["in", schema.kind, "minor_road", "minor", "tertiary"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.structure))),
                line_width: Some(linear_zoom_interpolation(&[
                    (11.0, 0.0),
                    (12.5, 0.5),
                    (15.0, 2.0),
                    (18.0, 11.0),
                ])),
                ..Default::default()
            },
        },
        // roads_tunnels_link
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is(Brunnel::Tunnel),
                schema.is_link()
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.structure))),
                line_width: Some(linear_zoom_interpolation(&[
                    (13.0, 0.0),
                    (13.5, 1.0),
                    (18.0, 11.0),
                ])),
                ..Default::default()
            },
        },
        // roads_tunnels_major
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is(Brunnel::Tunnel),
                ["in", schema.kind, "major_road", "primary", "secondary"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.structure))),
                line_width: Some(linear_zoom_interpolation(&[
                    (6.0, 0.0),
                    (12.0, 1.6),
                    (15.0, 3.0),
                    (18.0, 13.0),
                ])),
                ..Default::default()
            },
        },
        // roads_tunnels_highway
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is(Brunnel::Tunnel),
                ["==", ["get", schema.kind], "highway"],
                ["!", schema.is_link()]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.structure))),
                line_width: Some(linear_zoom_interpolation(&[
                    (3.0, 0.0),
                    (6.0, 1.65),
                    (12.0, 2.4),
                    (15.0, 7.5),
                    (18.0, 22.5),
                ])),
                ..Default::default()
            },
        },
        // buildings
        Layer::Fill {
            source_layer: schema.buildings.into(),
            filter: Some(Filter(json!([
                "in",
                schema.kind,
                "building",
                "building_part"
            ]))),
            paint: Paint {
                fill_color: Some(Color(json!(palette.structure))),
                fill_opacity: Some(Float(json!(0.5))),
                ..Default::default()
            },
        },
        // roads_pier
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!(["==", schema.kind_detail, "pier"]))),
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
        // roads_minor_service_casing
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is_not(Brunnel::Tunnel),
                schema.is_not(Brunnel::Bridge),
                ["in", schema.kind, "minor_road", "minor", "tertiary"],
                ["==", schema.kind_detail, "service"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.casing))),
                line_width: Some(linear_zoom_interpolation(&[(13.0, 0.0), (13.5, 0.8)])),
                ..Default::default()
            },
        },
        // roads_minor_casing
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is_not(Brunnel::Tunnel),
                schema.is_not(Brunnel::Bridge),
                ["in", schema.kind, "minor_road", "minor", "tertiary"],
                ["!=", schema.kind_detail, "service"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.casing))),
                line_width: Some(linear_zoom_interpolation(&[(12.0, 0.0), (12.5, 1.0)])),
                ..Default::default()
            },
        },
        // roads_link_casing
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!(schema.is_link()))),
            paint: Paint {
                line_color: Some(Color(json!(palette.casing))),
                line_width: Some(linear_zoom_interpolation(&[(13.0, 0.0), (13.5, 1.5)])),
                ..Default::default()
            },
        },
        // roads_major_casing_late
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is_not(Brunnel::Tunnel),
                schema.is_not(Brunnel::Bridge),
                ["in", schema.kind, "major_road", "primary", "secondary"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.casing))),
                line_width: Some(linear_zoom_interpolation(&[(9.0, 0.0), (9.5, 1.0)])),
                ..Default::default()
            },
        },
        // roads_highway_casing_late
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is_not(Brunnel::Tunnel),
                schema.is_not(Brunnel::Bridge),
                ["in", schema.kind, "highway", "motorway", "trunk"],
                schema.is_not_link()
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.casing))),
                line_width: Some(linear_zoom_interpolation(&[
                    (7.0, 0.0),
                    (7.5, 1.5),
                    (20.0, 22.5),
                ])),
                ..Default::default()
            },
        },
        // roads_other
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is_not(Brunnel::Tunnel),
                schema.is_not(Brunnel::Bridge),
                ["in", schema.kind, "other", "path", "service", "track"],
                ["!=", schema.kind_detail, "pier"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.structure))),
                line_width: Some(linear_zoom_interpolation(&[
                    (11.0, 0.0),
                    (12.0, 1.0),
                    (20.0, 7.0),
                ])),
                ..Default::default()
            },
        },
        // roads_link
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!(schema.is_link()))),
            paint: Paint {
                line_color: Some(Color(json!(palette.bridge))),
                line_width: Some(linear_zoom_interpolation(&[
                    (13.0, 0.0),
                    (13.5, 1.0),
                    (18.0, 11.0),
                ])),
                ..Default::default()
            },
        },
        // roads_minor
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is_not(Brunnel::Tunnel),
                schema.is_not(Brunnel::Bridge),
                ["in", schema.kind, "minor_road", "minor", "tertiary"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.road))),
                line_width: Some(linear_zoom_interpolation(&[
                    (11.0, 0.0),
                    (12.0, 1.0),
                    (20.0, 7.0),
                ])),
                ..Default::default()
            },
        },
        // roads_major_border
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is_not(Brunnel::Tunnel),
                schema.is_not(Brunnel::Bridge),
                ["in", schema.kind, "major_road", "primary", "secondary"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.major_road_border))),
                line_width: Some(linear_zoom_interpolation(&[
                    (6.0, 0.0),
                    (12.0, 1.6),
                    (15.0, 5.0),
                    (18.0, 17.0),
                ])),
                ..Default::default()
            },
        },
        // roads_major
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is_not(Brunnel::Tunnel),
                schema.is_not(Brunnel::Bridge),
                ["in", schema.kind, "major_road", "primary", "secondary"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.road))),
                line_width: Some(linear_zoom_interpolation(&[
                    (6.0, 0.0),
                    (12.0, 1.6),
                    (15.0, 3.0),
                    (18.0, 13.0),
                ])),
                ..Default::default()
            },
        },
        // roads_highway
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is_not(Brunnel::Tunnel),
                schema.is_not(Brunnel::Bridge),
                ["in", schema.kind, "highway", "motorway", "trunk"],
                schema.is_not_link()
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.highway))),
                line_width: Some(linear_zoom_interpolation(&[
                    (3.0, 0.0),
                    (6.0, 1.65),
                    (12.0, 2.4),
                    (15.0, 7.5),
                    (18.0, 22.5),
                ])),
                ..Default::default()
            },
        },
        // roads_rail
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!(["in", schema.kind, "rail", "transit"]))),
            paint: Paint {
                //line_opacity: Some(Float(json!(0.5))),
                line_color: Some(Color(json!(palette.rail))),
                line_width: Some(linear_zoom_interpolation(&[(3.0, 0.0), (18.0, 3.5)])),
                ..Default::default()
            },
        },
        // roads_rail_inside
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!(["in", schema.kind, "rail", "transit"]))),
            paint: Paint {
                line_opacity: Some(Float(json!(0.5))),
                line_color: Some(Color(json!(palette.rail_tie))),
                line_dasharray: Some(Dasharray(json!([4, 1]))),
                line_width: Some(linear_zoom_interpolation(&[(3.0, 0.0), (18.0, 2.0)])),
                ..Default::default()
            },
        },
        // boundaries_country
        Layer::Line {
            source_layer: schema.boundaries.into(),
            filter: Some(Filter(json!(["<=", schema.kind_detail, 2]))),
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
        // roads_bridges_other_casing
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is(Brunnel::Bridge),
                ["in", schema.kind, "other", "path", "service", "track"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.casing))),
                ..Default::default()
            },
        },
        // roads_bridges_link_casing
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is(Brunnel::Bridge),
                schema.is_link()
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.casing))),
                line_width: Some(linear_zoom_interpolation(&[(12.0, 0.0), (12.5, 1.5)])),
                ..Default::default()
            },
        },
        // roads_bridges_minor_casing
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is(Brunnel::Bridge),
                ["in", schema.kind, "minor_road", "minor", "tertiary"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.casing))),
                line_width: Some(linear_zoom_interpolation(&[(13.0, 0.0), (13.5, 0.8)])),
                ..Default::default()
            },
        },
        // roads_bridges_major_casing
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is(Brunnel::Bridge),
                ["in", schema.kind, "major_road", "primary", "secondary"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.casing))),
                line_width: Some(linear_zoom_interpolation(&[(9.0, 0.0), (9.5, 1.5)])),
                ..Default::default()
            },
        },
        // roads_bridges_other
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is(Brunnel::Bridge),
                ["in", schema.kind, "other", "path", "service", "track"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.bridge))),
                line_dasharray: Some(Dasharray(json!([2, 1]))),
                line_width: Some(linear_zoom_interpolation(&[(14.0, 0.0), (20.0, 7.0)])),
                ..Default::default()
            },
        },
        // roads_bridges_minor
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is(Brunnel::Bridge),
                ["in", schema.kind, "minor_road", "minor", "tertiary"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.bridge))),
                line_width: Some(linear_zoom_interpolation(&[
                    (11.0, 0.0),
                    (12.5, 0.5),
                    (15.0, 2.0),
                    (18.0, 11.0),
                ])),
                ..Default::default()
            },
        },
        // roads_bridges_link
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is(Brunnel::Bridge),
                schema.is_link()
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.bridge))),
                line_width: Some(linear_zoom_interpolation(&[
                    (13.0, 0.0),
                    (13.5, 1.0),
                    (18.0, 11.0),
                ])),
                ..Default::default()
            },
        },
        // roads_bridges_major
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is(Brunnel::Bridge),
                ["in", schema.kind, "major_road", "primary", "secondary"]
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.structure))),
                line_width: Some(linear_zoom_interpolation(&[
                    (6.0, 0.0),
                    (12.0, 1.6),
                    (15.0, 3.0),
                    (18.0, 13.0),
                ])),
                ..Default::default()
            },
        },
        // roads_bridges_highway_casing
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is(Brunnel::Bridge),
                ["in", schema.kind, "highway", "motorway", "trunk"],
                schema.is_not_link()
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.casing))),
                line_width: Some(linear_zoom_interpolation(&[
                    (7.0, 0.0),
                    (7.5, 1.5),
                    (20.0, 22.5),
                ])),
                ..Default::default()
            },
        },
        // roads_bridges_highway
        Layer::Line {
            source_layer: schema.roads.into(),
            filter: Some(Filter(json!([
                "all",
                schema.is(Brunnel::Bridge),
                ["in", schema.kind, "highway", "motorway", "trunk"],
                schema.is_not_link()
            ]))),
            paint: Paint {
                line_color: Some(Color(json!(palette.structure))),
                line_width: Some(linear_zoom_interpolation(&[
                    (3.0, 0.0),
                    (6.0, 1.65),
                    (12.0, 2.4),
                    (15.0, 7.5),
                    (18.0, 22.5),
                ])),
                ..Default::default()
            },
        },
        // address_label
        Layer::Symbol {
            source_layer: schema.buildings.into(),
            filter: Some(Filter(json!(["==", schema.kind, "address"]))),
            layout: Layout {
                text_field: Some(json!(["get", "addr_housenumber"])),
                text_size: Some(Float(json!(10))),
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.label))),
                text_halo_color: Some(Color(json!(palette.casing))),
                ..Default::default()
            }),
        },
        // water_waterway_label
        Layer::Symbol {
            source_layer: schema.waterway.into(),
            filter: Some(Filter(json!(["in", schema.kind, "river", "stream"]))),
            layout: Layout {
                text_field: Some(json!(["get", "name"])),
                text_size: Some(Float(json!(12))),
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.label))),
                text_halo_color: Some(Color(json!(palette.muted))),
                ..Default::default()
            }),
        },
        // roads_oneway
        Layer::Symbol {
            source_layer: schema.road_labels.into(),
            filter: Some(Filter(json!(["==", ["get", "oneway"], "yes"]))),
            layout: Layout {
                text_field: None,
                text_size: None,
            },
            paint: None,
        },
        // roads_labels_minor
        Layer::Symbol {
            source_layer: schema.road_labels.into(),
            filter: Some(Filter(json!([
                "in",
                schema.kind,
                "minor_road",
                "other",
                "path"
            ]))),
            layout: Layout {
                text_field: Some(json!(["get", "name"])),
                text_size: Some(Float(json!(12))),
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.label_muted))),
                text_halo_color: Some(Color(json!(palette.background))),
                ..Default::default()
            }),
        },
        // water_label_ocean
        Layer::Symbol {
            source_layer: schema.water.into(),
            filter: Some(Filter(json!([
                "in",
                schema.kind,
                "sea",
                "ocean",
                "bay",
                "strait",
                "fjord"
            ]))),
            layout: Layout {
                text_field: Some(json!(["get", "name"])),
                text_size: Some(linear_zoom_interpolation(&[(3.0, 10.0), (10.0, 12.0)])),
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.label))),
                text_halo_color: Some(Color(json!(palette.muted))),
                ..Default::default()
            }),
        },
        // earth_label_islands
        Layer::Symbol {
            source_layer: schema.earth.into(),
            filter: Some(Filter(json!(["in", schema.kind, "island"]))),
            layout: Layout {
                text_field: Some(json!(["get", "name"])),
                text_size: Some(Float(json!(10))),
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.label_muted))),
                text_halo_color: Some(Color(json!(palette.casing))),
                ..Default::default()
            }),
        },
        // water_label_lakes
        Layer::Symbol {
            source_layer: schema.water.into(),
            filter: Some(Filter(json!(["in", schema.kind, "lake", "water"]))),
            layout: Layout {
                text_field: Some(json!(["get", "name"])),
                text_size: Some(linear_zoom_interpolation(&[
                    (3.0, 10.0),
                    (6.0, 12.0),
                    (10.0, 12.0),
                ])),
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.label))),
                text_halo_color: Some(Color(json!(palette.muted))),
                ..Default::default()
            }),
        },
        // roads_shields
        Layer::Symbol {
            source_layer: schema.road_labels.into(),
            filter: Some(Filter(json!([
                "all",
                [
                    "in",
                    ["get", schema.kind],
                    ["literal", ["highway", "major_road"]]
                ],
                ["has", "shield_text"],
                ["<=", ["length", ["get", "shield_text"]], 5]
            ]))),
            layout: Layout {
                text_field: Some(json!(["get", "shield_text"])),
                text_size: Some(Float(json!(8))),
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.label_muted))),
                ..Default::default()
            }),
        },
        // roads_labels_major
        Layer::Symbol {
            source_layer: schema.road_labels.into(),
            filter: Some(Filter(json!(["in", schema.kind, "highway", "major_road"]))),
            layout: Layout {
                text_field: Some(json!(["get", "name"])),
                text_size: Some(Float(json!(13))),
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.label))),
                text_halo_color: Some(Color(json!(palette.background))),
                ..Default::default()
            }),
        },
        // places_subplace
        Layer::Symbol {
            source_layer: schema.places.into(),
            filter: Some(Filter(json!([
                "in",
                schema.kind,
                "neighbourhood",
                "macrohood"
            ]))),
            layout: Layout {
                text_field: Some(json!(["get", "name"])),
                text_size: Some(linear_zoom_interpolation(&[
                    (11.0, 8.0),
                    (14.0, 14.0),
                    (18.0, 24.0),
                ])),
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.label_muted))),
                text_halo_color: Some(Color(json!(palette.casing))),
                ..Default::default()
            }),
        },
        // places_region
        Layer::Symbol {
            source_layer: schema.places.into(),
            filter: Some(Filter(json!(["==", schema.kind, "region"]))),
            layout: Layout {
                text_field: Some(json!(["get", "name"])),
                text_size: Some(linear_zoom_interpolation(&[(3.0, 11.0), (7.0, 16.0)])),
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.label))),
                text_halo_color: Some(Color(json!(palette.casing))),
                ..Default::default()
            }),
        },
        // places_locality
        Layer::Symbol {
            source_layer: schema.places.into(),
            filter: Some(Filter(json!(["==", schema.kind, "locality"]))),
            layout: Layout {
                text_field: Some(json!(["get", "name"])),
                text_size: Some(Float(json!([
                    "interpolate",
                    ["linear"],
                    ["zoom"],
                    2,
                    [
                        "case",
                        ["<", ["get", schema.place_rank], 13],
                        8,
                        [">=", ["get", schema.place_rank], 13],
                        13,
                        0
                    ],
                    4,
                    [
                        "case",
                        ["<", ["get", schema.place_rank], 13],
                        10,
                        [">=", ["get", schema.place_rank], 13],
                        15,
                        0
                    ],
                    6,
                    [
                        "case",
                        ["<", ["get", schema.place_rank], 12],
                        11,
                        [">=", ["get", schema.place_rank], 12],
                        17,
                        0
                    ],
                    8,
                    [
                        "case",
                        ["<", ["get", schema.place_rank], 11],
                        11,
                        [">=", ["get", schema.place_rank], 11],
                        18,
                        0
                    ],
                    10,
                    [
                        "case",
                        ["<", ["get", schema.place_rank], 9],
                        12,
                        [">=", ["get", schema.place_rank], 9],
                        20,
                        0
                    ],
                    15,
                    [
                        "case",
                        ["<", ["get", schema.place_rank], 8],
                        12,
                        [">=", ["get", schema.place_rank], 8],
                        22,
                        0
                    ]
                ]))),
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.locality_text))),
                text_halo_color: Some(Color(json!(palette.casing))),
                ..Default::default()
            }),
        },
        // places_country
        Layer::Symbol {
            source_layer: schema.places.into(),
            filter: Some(Filter(json!(["==", schema.kind, "country"]))),
            layout: Layout {
                text_field: Some(json!(["get", "name:en"])),
                text_size: Some(Float(json!([
                    "interpolate",
                    ["linear"],
                    ["zoom"],
                    2,
                    [
                        "case",
                        ["<", ["get", schema.place_rank], 10],
                        8,
                        [">=", ["get", schema.place_rank], 10],
                        12,
                        0
                    ],
                    6,
                    [
                        "case",
                        ["<", ["get", schema.place_rank], 8],
                        10,
                        [">=", ["get", schema.place_rank], 8],
                        18,
                        0
                    ],
                    8,
                    [
                        "case",
                        ["<", ["get", schema.place_rank], 7],
                        11,
                        [">=", ["get", schema.place_rank], 7],
                        20,
                        0
                    ]
                ]))),
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.label))),
                text_halo_color: Some(Color(json!(palette.casing))),
                ..Default::default()
            }),
        },
        // stations
        Layer::Symbol {
            source_layer: schema.pois.into(),
            filter: Some(Filter(json!(["==", schema.kind, "station"]))),
            layout: Layout {
                text_field: Some(json!(["get", "name"])),
                text_size: Some(Float(json!(11))),
            },
            paint: Some(Paint {
                text_color: Some(Color(json!(palette.station))),
                text_halo_color: Some(Color(json!(palette.casing))),
                ..Default::default()
            }),
        },
    ];

    // A schema which does not carry a layer leaves its name empty, and an empty source layer
    // would match every layer of the tile rather than none.
    layers.retain(|layer| source_layer_of(layer).is_none_or(|source| !source.is_all()));

    Style { layers }
}

fn source_layer_of(layer: &Layer) -> Option<&SourceLayer> {
    match layer {
        Layer::Fill { source_layer, .. }
        | Layer::Line { source_layer, .. }
        | Layer::Symbol { source_layer, .. } => Some(source_layer),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Shade {
    Light,
    Dark,
}

impl Shade {
    /// In the order they are offered.
    pub const ALL: [Shade; 2] = [Shade::Light, Shade::Dark];

    pub fn name(self) -> &'static str {
        match self {
            Shade::Light => "light",
            Shade::Dark => "dark",
        }
    }
}

pub fn style(shade: Shade, schema: Schema) -> Style {
    build(
        match shade {
            Shade::Light => &LIGHT,
            Shade::Dark => &DARK,
        },
        schema,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asks_for(style: &Style, name: &str) -> bool {
        style
            .layers
            .iter()
            .filter_map(source_layer_of)
            .any(|source| source.matches(name))
    }

    /// OpenMapTiles has no land polygon, so those layers are dropped rather than asked for.
    #[test]
    fn a_schema_only_gets_the_layers_it_can_serve() {
        assert!(asks_for(&style(Shade::Light, PROTOMAPS), "earth"));
        assert!(!asks_for(&style(Shade::Light, OPENMAPTILES), "earth"));
    }

    #[test]
    fn both_shades_draw_the_same_layers() {
        for schema in [PROTOMAPS, OPENMAPTILES] {
            assert_eq!(
                style(Shade::Dark, schema).layers.len(),
                style(Shade::Light, schema).layers.len()
            );
        }
    }

    /// OpenMapTiles spreads landuse over `landcover`, `landuse` and `park`.
    #[test]
    fn a_concept_split_across_layers_needs_no_extra_layers() {
        let openmaptiles = style(Shade::Light, OPENMAPTILES);

        for name in ["landcover", "landuse", "park"] {
            assert!(asks_for(&openmaptiles, name), "does not ask for {name}");
        }

        let landuse_layers = |style: &Style| {
            style
                .layers
                .iter()
                .filter_map(source_layer_of)
                .filter(|source| source.matches("landcover") || source.matches("landuse"))
                .count()
        };

        assert_eq!(
            landuse_layers(&openmaptiles),
            landuse_layers(&style(Shade::Light, PROTOMAPS))
        );
    }

    #[test]
    fn each_schema_asks_for_its_own_source_layers() {
        let protomaps = style(Shade::Light, PROTOMAPS);
        let openmaptiles = style(Shade::Light, OPENMAPTILES);

        assert!(asks_for(&protomaps, "roads"));
        assert!(asks_for(&protomaps, "buildings"));
        assert!(!asks_for(&protomaps, "transportation"));

        assert!(asks_for(&openmaptiles, "transportation"));
        assert!(asks_for(&openmaptiles, "transportation_name"));
        assert!(asks_for(&openmaptiles, "waterway"));
        assert!(asks_for(&openmaptiles, "building"));
        assert!(!asks_for(&openmaptiles, "roads"));
    }

    /// A layer matching everything scans every layer of a tile, ruinous for a basemap.
    #[test]
    fn no_layer_matches_everything() {
        for schema in [PROTOMAPS, OPENMAPTILES] {
            let style = style(Shade::Light, schema);
            assert!(
                style
                    .layers
                    .iter()
                    .filter_map(source_layer_of)
                    .all(|source| !source.is_all())
            );
        }
    }
}
