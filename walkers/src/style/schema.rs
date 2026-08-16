//! What a source calls the things it carries.

use super::{Value, json};

/// Which vector tile schema the tiles follow.
///
/// Only the names live here. Differing property values are absorbed by the filters, which is
/// not possible for layer names because `source_layer` is read before there is any feature.
#[derive(Clone, Copy)]
pub struct Schema {
    pub earth: &'static str,
    pub landuse: &'static [&'static str],
    pub water: &'static str,
    pub water_labels: &'static [&'static str],
    pub waterway: &'static str,
    pub roads: &'static str,
    pub road_labels: &'static str,
    pub buildings: &'static str,
    pub places: &'static str,
    pub pois: &'static str,
    pub peaks: &'static [&'static str],
    pub boundaries: &'static str,
    pub kind: &'static str,
    pub kind_detail: &'static str,
    pub admin_level: &'static str,
    pub place_rank: &'static str,
    pub brunnel: Option<&'static str>,
    pub link: &'static str,
}

pub const PROTOMAPS: Schema = Schema {
    earth: "earth",
    landuse: &["landuse"],
    water: "water",
    water_labels: &["water"],
    waterway: "water",
    roads: "roads",
    road_labels: "roads",
    buildings: "buildings",
    places: "places",
    pois: "pois",
    peaks: &["pois"],
    boundaries: "boundaries",
    kind: "kind",
    kind_detail: "kind_detail",
    admin_level: "kind_detail",
    place_rank: "population_rank",
    brunnel: None,
    link: "is_link",
};

/// `earth` is empty because there is no land polygon here; the background stands in for it.
pub const OPENMAPTILES: Schema = Schema {
    earth: "",
    landuse: &["landcover", "landuse", "park"],
    water: "water",
    water_labels: &["water_name"],
    waterway: "waterway",
    roads: "transportation",
    road_labels: "transportation_name",
    buildings: "building",
    places: "place",
    pois: "poi",
    peaks: &["mountain_peak"],
    boundaries: "boundary",
    kind: "class",
    kind_detail: "subclass",
    admin_level: "admin_level",
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
    pub fn is(&self, what: Brunnel) -> Value {
        match self.brunnel {
            Some(key) => json!(["==", key, what.openmaptiles()]),
            None => json!(["has", what.protomaps()]),
        }
    }

    pub fn is_not(&self, what: Brunnel) -> Value {
        match self.brunnel {
            Some(key) => json!(["!=", key, what.openmaptiles()]),
            None => json!(["!has", what.protomaps()]),
        }
    }

    pub fn is_link(&self) -> Value {
        match self.brunnel {
            Some(_) => json!(["==", self.link, 1]),
            None => json!(["has", self.link]),
        }
    }

    pub fn is_not_link(&self) -> Value {
        match self.brunnel {
            Some(_) => json!(["!=", self.link, 1]),
            None => json!(["!has", self.link]),
        }
    }
}
