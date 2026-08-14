//! Reading and writing the journal file.
//!
//! The journal is a GeoJSON `FeatureCollection` of `Point` features. Its `properties` have no
//! schema, so the format can gain fields without migrations, and anything we do not understand
//! survives a load-save round trip untouched.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::PathBuf;

use geojson::{Feature, FeatureCollection, GeoJson, Geometry, JsonObject};
use walkers::{Position, lon_lat};

const NAME: &str = "name";

/// A single place in the journal.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub position: Position,
    pub name: String,

    /// Properties we do not understand, kept so that saving does not destroy them.
    unknown_properties: JsonObject,
}

impl Place {
    pub fn new(position: Position, name: impl Into<String>) -> Self {
        Self {
            position,
            name: name.into(),
            unknown_properties: JsonObject::new(),
        }
    }

    /// Features which are not points - lines, polygons - are not places, and are skipped.
    fn from_feature(feature: Feature) -> Option<Self> {
        let coordinates = match feature.geometry.map(|geometry| geometry.value) {
            Some(geojson::GeometryValue::Point { coordinates }) => coordinates,
            _ => return None,
        };

        // GeoJSON is longitude first, which is also what `walkers` expects.
        let (longitude, latitude) = match coordinates.as_slice() {
            [longitude, latitude, ..] => (*longitude, *latitude),
            _ => return None,
        };

        let mut properties = feature.properties.unwrap_or_default();

        let name = properties
            .remove(NAME)
            .and_then(|name| match name {
                serde_json::Value::String(name) => Some(name),
                // A number is still a reasonable label, whereas `null` is not.
                other => (!other.is_null()).then(|| other.to_string()),
            })
            .unwrap_or_default();

        Some(Self {
            position: lon_lat(longitude, latitude),
            name,
            unknown_properties: properties,
        })
    }

    fn to_feature(&self) -> Feature {
        let mut properties = self.unknown_properties.clone();
        properties.insert(NAME.to_owned(), self.name.to_owned().into());

        Feature {
            geometry: Some(Geometry::new_point([self.position.x(), self.position.y()])),
            properties: Some(properties),
            ..Default::default()
        }
    }
}

/// All the places, and where they came from.
pub struct Journal {
    pub path: PathBuf,
    pub places: Vec<Place>,
}

impl Journal {
    /// A file which is not there yet is an empty journal, not an error.
    pub fn load(path: impl Into<PathBuf>) -> Result<Self, Error> {
        let path = path.into();

        let places = match fs::read_to_string(&path) {
            Ok(contents) => parse(&contents)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(Error::Io(error)),
        };

        Ok(Self { path, places })
    }

    /// Write the journal, so that it either fully lands, or is not touched at all.
    pub fn save(&self) -> Result<(), Error> {
        if let Some(directory) = self.path.parent() {
            fs::create_dir_all(directory)?;
        }

        let contents = serialize(&self.places);

        // Renaming over the old file is atomic, so a crash mid-save leaves the previous
        // journal intact instead of a half-written one.
        let temporary = self.path.with_extension("geojson.saving");
        let mut file = File::create(&temporary)?;
        file.write_all(contents.as_bytes())?;

        // Both syncs matter: the first one so that the bytes are on the disk before anything
        // points at them, the second one so that the rename itself survives a power loss.
        file.sync_all()?;
        fs::rename(&temporary, &self.path)?;
        if let Some(directory) = self.path.parent() {
            File::open(directory)?.sync_all()?;
        }

        Ok(())
    }
}

fn parse(contents: &str) -> Result<Vec<Place>, Error> {
    // An empty file is an empty journal, which is friendlier than a parse error for something
    // that a crashed editor or a `touch` can easily leave behind.
    if contents.trim().is_empty() {
        return Ok(Vec::new());
    }

    let features = match contents.parse::<GeoJson>()? {
        GeoJson::FeatureCollection(collection) => collection.features,
        GeoJson::Feature(feature) => vec![feature],
        GeoJson::Geometry(geometry) => vec![Feature {
            geometry: Some(geometry),
            ..Default::default()
        }],
    };

    Ok(features
        .into_iter()
        .filter_map(Place::from_feature)
        .collect())
}

fn serialize(places: &[Place]) -> String {
    let collection = FeatureCollection {
        bbox: None,
        features: places.iter().map(Place::to_feature).collect(),
        foreign_members: None,
    };

    // Pretty printed, because a journal is something one might want to read, diff or edit by
    // hand.
    format!("{:#}\n", GeoJson::FeatureCollection(collection))
}

/// Where the journal lives, unless told otherwise.
pub fn default_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::home_dir().map(|home| home.join(".local").join("share")))?;
    Some(base.join("wanderers").join("journal.geojson"))
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not read or write the journal: {0}")]
    Io(#[from] io::Error),

    #[error("the journal is not valid GeoJSON: {0}")]
    Malformed(#[from] geojson::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "type": "FeatureCollection",
        "features": [
            {
                "type": "Feature",
                "geometry": { "type": "Point", "coordinates": [17.0321, 51.1101] },
                "properties": { "name": "Rynek" }
            },
            {
                "type": "Feature",
                "geometry": { "type": "Point", "coordinates": [-58.4438, -34.6083] },
                "properties": { "name": "Buenos Aires" }
            }
        ]
    }"#;

    #[test]
    fn places_are_read_from_a_feature_collection() {
        let places = parse(SAMPLE).unwrap();

        assert_eq!(places.len(), 2);
        assert_eq!(places[0].name, "Rynek");
        assert_eq!(places[0].position, lon_lat(17.0321, 51.1101));
        assert_eq!(places[1].name, "Buenos Aires");
    }

    #[test]
    fn a_missing_file_is_an_empty_journal() {
        let journal = Journal::load("there/is/no/such/journal.geojson").unwrap();
        assert!(journal.places.is_empty());
    }

    #[test]
    fn an_empty_file_is_an_empty_journal() {
        assert!(parse("").unwrap().is_empty());
        assert!(parse("   \n ").unwrap().is_empty());
    }

    #[test]
    fn places_survive_a_round_trip() {
        let places = parse(SAMPLE).unwrap();
        assert_eq!(parse(&serialize(&places)).unwrap(), places);
    }

    #[test]
    fn saving_creates_a_journal_which_can_be_read_back() {
        let path = std::env::temp_dir()
            .join(format!("wanderers-{}", std::process::id()))
            .join("journal.geojson");

        let journal = Journal {
            path: path.to_owned(),
            places: vec![Place::new(lon_lat(17.0321, 51.1101), "Rynek")],
        };
        journal.save().unwrap();

        assert_eq!(Journal::load(&path).unwrap().places, journal.places);

        // Saving again has to overwrite the journal, not append to it.
        journal.save().unwrap();
        assert_eq!(Journal::load(&path).unwrap().places.len(), 1);

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    /// The whole point of picking a schema-less format: a file written by a version of the app
    /// which knows more than this one must not come out of it damaged.
    #[test]
    fn properties_of_a_newer_version_are_not_dropped() {
        let from_the_future = r#"{
            "type": "FeatureCollection",
            "features": [{
                "type": "Feature",
                "geometry": { "type": "Point", "coordinates": [17.0321, 51.1101] },
                "properties": {
                    "name": "Rynek",
                    "status": "visited",
                    "photos": ["rynek.jpg"],
                    "rating": 5
                }
            }]
        }"#;

        let saved = serialize(&parse(from_the_future).unwrap());
        let json: serde_json::Value = serde_json::from_str(&saved).unwrap();
        let properties = &json["features"][0]["properties"];

        assert_eq!(properties["status"], "visited");
        assert_eq!(properties["photos"][0], "rynek.jpg");
        assert_eq!(properties["rating"], 5);
        assert_eq!(properties["name"], "Rynek");
    }

    #[test]
    fn places_can_have_no_name() {
        let places = parse(
            r#"{
                "type": "FeatureCollection",
                "features": [{
                    "type": "Feature",
                    "geometry": { "type": "Point", "coordinates": [17.0321, 51.1101] },
                    "properties": {}
                }]
            }"#,
        )
        .unwrap();

        assert_eq!(places.len(), 1);
        assert_eq!(places[0].name, "");
    }

    /// Journals made elsewhere are unlikely to consist of points only.
    #[test]
    fn features_which_are_not_points_are_skipped() {
        let places = parse(
            r#"{
                "type": "FeatureCollection",
                "features": [
                    {
                        "type": "Feature",
                        "geometry": {
                            "type": "LineString",
                            "coordinates": [[17.0, 51.0], [17.1, 51.1]]
                        },
                        "properties": { "name": "a walk" }
                    },
                    {
                        "type": "Feature",
                        "geometry": { "type": "Point", "coordinates": [17.0321, 51.1101] },
                        "properties": { "name": "Rynek" }
                    }
                ]
            }"#,
        )
        .unwrap();

        assert_eq!(places.len(), 1);
        assert_eq!(places[0].name, "Rynek");
    }

    #[test]
    fn garbage_is_reported_as_such() {
        assert!(matches!(parse("not json at all"), Err(Error::Malformed(_))));
    }
}
