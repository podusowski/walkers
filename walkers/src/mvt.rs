//! Renderer for Mapbox Vector Tiles.

use std::collections::HashMap;

use egui::{
    Color32, Pos2, Shape, Stroke,
    emath::TSTransform,
    epaint::{PathShape, PathStroke},
    pos2,
};
use geo_types::Geometry;
use log::warn;
use mvt_reader::feature::{Feature, Value};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Mvt(#[from] mvt_reader::error::ParserError),
    #[error("Layer not found: {0}. Available layers: {1:?}")]
    LayerNotFound(String, Vec<String>),
    #[error("Unsupported layer extent: {0}")]
    UnsupportedLayerExtent(String),
    #[error("Unsupported kind: {0:?}")]
    UnsupportedFeatureKind(HashMap<String, Value>),
    #[error("Missing kind in properties: {0:?}")]
    FeatureWithoutKind(HashMap<String, Value>),
    #[error("Missing properties in feature")]
    FeatureWithoutProperties,
}

/// Currently this is the only supported extent.
const ONLY_SUPPORTED_EXTENT: u32 = 4096;

#[derive(Debug, Clone)]
pub enum ShapeOrText {
    Shape(Shape),
    Text {
        position: Pos2,
        text: String,
        font_size: f32,
    },
}

impl From<Shape> for ShapeOrText {
    fn from(shape: Shape) -> Self {
        ShapeOrText::Shape(shape)
    }
}

impl ShapeOrText {
    pub fn transform(&mut self, transform: TSTransform) {
        match self {
            ShapeOrText::Shape(shape) => {
                shape.transform(transform);
            }
            ShapeOrText::Text { position, .. } => {
                *position *= transform.scaling;
                *position += transform.translation;
            }
        }
    }
}

/// Render MVT data into a list of [`epaint::Shape`]s.
pub fn render(data: &mvt_reader::Reader) -> Result<Vec<ShapeOrText>, Error> {
    let mut shapes = Vec::new();

    let known_layers = [
        "earth",
        "water",
        "landuse",
        "landcover",
        "buildings",
        "roads",
        "places",
        "pois",
    ];

    for layer in data.get_layer_names()? {
        if !known_layers.contains(&layer.as_str()) {
            warn!("Unknown layer '{layer}' found. Skipping.");
        }
    }

    for layer in known_layers {
        if let Ok(layer_index) = find_layer(data, layer) {
            for feature in data.get_features(layer_index)? {
                if let Err(err) = feature_into_shape(&feature, &mut shapes) {
                    warn!("{err}");
                }
            }
        } else {
            warn!("Layer '{layer}' not found. Skipping.");
        }
    }

    Ok(shapes)
}

/// Transform shapes from MVT space to screen space.
pub fn transformed(shapes: &[ShapeOrText], rect: egui::Rect) -> Vec<ShapeOrText> {
    let transform = TSTransform {
        scaling: rect.width() / ONLY_SUPPORTED_EXTENT as f32,
        translation: rect.min.to_vec2(),
    };

    let mut result = shapes.to_vec();
    for shape in result.iter_mut() {
        shape.transform(transform);
    }
    result
}

fn feature_into_shape(feature: &Feature, shapes: &mut Vec<ShapeOrText>) -> Result<(), Error> {
    let properties = feature
        .properties
        .as_ref()
        .ok_or(Error::FeatureWithoutProperties)?;
    match &feature.geometry {
        Geometry::LineString(line_string) => {
            if let Some(stroke) = line_stroke(properties)? {
                let points = line_string
                    .0
                    .iter()
                    .map(|p| pos2(p.x, p.y))
                    .collect::<Vec<_>>();
                shapes.push(Shape::line(points, stroke).into());
            }
        }
        Geometry::MultiLineString(multi_line_string) => {
            if let Some(stroke) = line_stroke(properties)? {
                for line_string in multi_line_string {
                    let points = line_string
                        .0
                        .iter()
                        .map(|p| pos2(p.x, p.y))
                        .collect::<Vec<_>>();
                    shapes.push(Shape::line(points, stroke).into());
                }
            }
        }
        Geometry::MultiPoint(multi_point) => shapes.extend(points(
            properties,
            &multi_point
                .0
                .iter()
                .map(|p| pos2(p.x(), p.y()))
                .collect::<Vec<_>>(),
        )?),
        Geometry::MultiPolygon(multi_polygon) => {
            if let Some(fill) = polygon_fill(properties)? {
                for polygon in multi_polygon.iter() {
                    let points = polygon
                        .exterior()
                        .0
                        .iter()
                        .map(|p| pos2(p.x, p.y))
                        .collect::<Vec<_>>();
                    let holes = polygon
                        .interiors()
                        .iter()
                        .map(|hole| hole.0.iter().map(|p| pos2(p.x, p.y)).collect::<Vec<_>>())
                        .collect::<Vec<_>>();
                    shapes.extend(
                        arbitrary_polygon(&points, &holes, fill)
                            .into_iter()
                            .map(Into::into),
                    );
                }
            }
        }
        Geometry::Point(_point) => todo!(),
        Geometry::Line(_line) => todo!(),
        Geometry::Polygon(_polygon) => todo!(),
        Geometry::GeometryCollection(_geometry_collection) => todo!(),
        Geometry::Rect(_rect) => todo!(),
        Geometry::Triangle(_triangle) => todo!(),
    }
    Ok(())
}

const WATER_COLOR: Color32 = Color32::from_rgb(12, 39, 77);
const ROAD_COLOR: Color32 = Color32::from_rgb(80, 80, 80);

fn kind(properties: &HashMap<String, Value>) -> Result<String, Error> {
    if let Some(Value::String(kind)) = properties.get("kind") {
        Ok(kind.clone())
    } else {
        Err(Error::FeatureWithoutKind(properties.clone()))
    }
}

fn points(properties: &HashMap<String, Value>, points: &[Pos2]) -> Result<Vec<ShapeOrText>, Error> {
    let font_size = match kind(properties)?.as_str() {
        "neighbourhood" | "locality" => Ok(16.0),
        _ => Err(Error::UnsupportedFeatureKind(properties.clone())),
    }?;

    if let Some(Value::String(name)) = properties.get("name") {
        Ok(points
            .into_iter()
            .map(|point| ShapeOrText::Text {
                position: *point,
                text: name.clone(),
                font_size,
            })
            .collect::<Vec<_>>())
    } else {
        // Without name, there is currently nothing to render.
        Ok(Vec::new())
    }
}

fn polygon_fill(properties: &HashMap<String, Value>) -> Result<Option<Color32>, Error> {
    Ok(match kind(properties)?.as_str() {
        "water" | "fountain" | "swimming_pool" | "basin" | "lake" | "ditch" | "ocean" => {
            Some(WATER_COLOR)
        }
        "grass" | "garden" | "playground" | "zoo" | "park" | "forest" | "wood"
        | "village_green" | "scrub" | "grassland" | "allotments" | "pitch" | "dog_park"
        | "meadow" | "wetland" | "cemetery" | "golf_course" | "nature_reserve"
        | "national_park" | "island" => Some(Color32::from_rgb(10, 20, 0)),
        "farmland" => Some(Color32::from_rgb(20, 25, 0)),
        "building" | "building_part" | "pier" | "runway" | "bare_rock" => {
            Some(Color32::from_rgb(30, 30, 30))
        }
        "military" => Some(Color32::from_rgb(46, 31, 31)),
        "sand" | "beach" => Some(Color32::from_rgb(64, 64, 0)),
        "pedestrian" | "recreation_ground" | "railway" | "industrial" | "residential"
        | "commercial" | "protected_area" | "school" | "platform" | "kindergarten" | "cliff"
        | "university" | "hospital" | "college" | "aerodrome" | "earth" => None,
        other => {
            warn!("Unknown polygon kind: {other}");
            Some(Color32::RED)
        }
    })
}

fn line_stroke(properties: &HashMap<String, Value>) -> Result<Option<Stroke>, Error> {
    Ok(match kind(properties)?.as_str() {
        "highway" | "aeroway" => Some(Stroke::new(15.0, ROAD_COLOR)),
        "major_road" => Some(Stroke::new(12.0, ROAD_COLOR)),
        "minor_road" => Some(Stroke::new(9.0, ROAD_COLOR)),
        "rail" => Some(Stroke::new(3.0, ROAD_COLOR)),
        "path" => Some(Stroke::new(3.0, Color32::from_rgb(60, 40, 0))),
        "river" | "stream" | "drain" | "ditch" | "canal" => Some(Stroke::new(3.0, WATER_COLOR)),
        "other" | "aerialway" | "cliff" => None,
        other => {
            warn!("Unknown line kind: {other}");
            Some(Stroke::new(10.0, Color32::RED))
        }
    })
}

fn find_layer(data: &mvt_reader::Reader, name: &str) -> Result<usize, Error> {
    let layer = data
        .get_layer_metadata()?
        .into_iter()
        .find(|layer| layer.name == name);

    let Some(layer) = layer else {
        return Err(Error::LayerNotFound(
            name.to_string(),
            data.get_layer_names()?,
        ));
    };

    if layer.extent != ONLY_SUPPORTED_EXTENT {
        return Err(Error::UnsupportedLayerExtent(name.to_string()));
    }

    Ok(layer.layer_index)
}

/// Egui can only draw convex polygons, so we need to triangulate arbitrary ones.
fn arbitrary_polygon(exterior: &[Pos2], holes: &[Vec<Pos2>], fill: Color32) -> Vec<Shape> {
    let mut triangles = Vec::<usize>::new();

    // Prepare Earcut data by flattening exterior points...
    let mut all_points = Vec::new();
    all_points.extend(exterior.iter().map(|p| [p.x, p.y]));

    // ...and adding hole points while recording their indices.
    let mut hole_indices = Vec::new();
    for hole in holes {
        hole_indices.push(all_points.len());
        all_points.extend(hole.iter().map(|p| [p.x, p.y]));
    }

    earcut::Earcut::new().earcut(all_points.to_vec(), &hole_indices, &mut triangles);

    // Convert back to Pos2 for indexing
    let all_pos2: Vec<Pos2> = all_points.iter().map(|p| pos2(p[0], p[1])).collect();

    let mut shapes = Vec::new();
    for triangle_indices in triangles.chunks(3) {
        let triangle = [
            all_pos2[triangle_indices[0]],
            all_pos2[triangle_indices[1]],
            all_pos2[triangle_indices[2]],
        ];

        if triangle_area(triangle[0], triangle[1], triangle[2]) < 100.0 {
            // Too small to render without artifacts.
            continue;
        }

        shapes.push(PathShape::convex_polygon(triangle.to_vec(), fill, PathStroke::NONE).into());
    }
    shapes
}

fn triangle_area(a: Pos2, b: Pos2, c: Pos2) -> f32 {
    ((a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y)) / 2.0).abs()
}
