//! Renderer for Mapbox Vector Tiles.

use std::collections::HashMap;

use egui::{
    Color32, FontId, Pos2, Shape, Stroke,
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
    #[error("Unsupported feature kind with properties: {0:?}")]
    UnsupportedFeatureKind(HashMap<String, Value>),
    #[error("Missing kind in properties: {0:?}")]
    FeatureWithoutKind(HashMap<String, Value>),
    #[error("Missing properties in feature")]
    FeatureWithoutProperties,
}

/// Currently this is the only supported extent.
const ONLY_SUPPORTED_EXTENT: u32 = 4096;

/// Render MVT data into a list of [`epaint::Shape`]s.
pub fn render(data: &mvt_reader::Reader, egui_ctx: &egui::Context) -> Result<Vec<Shape>, Error> {
    let mut shapes = Vec::new();

    let known_layers = ["earth", "landuse", "water", "buildings", "roads", "places"];

    for layer in data.get_layer_names()? {
        if !known_layers.contains(&layer.as_str()) {
            warn!("Unknown layer '{layer}' found. Skipping.");
        }
    }

    for layer in known_layers {
        if let Ok(layer_index) = find_layer(data, layer) {
            for feature in data.get_features(layer_index)? {
                if let Err(err) = render_feature(&feature, &mut shapes, egui_ctx) {
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
pub fn transformed(shapes: &[Shape], rect: egui::Rect) -> Vec<Shape> {
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

fn render_feature(
    feature: &Feature,
    shapes: &mut Vec<Shape>,
    egui_ctx: &egui::Context,
) -> Result<(), Error> {
    let properties = feature
        .properties
        .as_ref()
        .ok_or(Error::FeatureWithoutProperties)?;
    match &feature.geometry {
        Geometry::Point(_point) => todo!(),
        Geometry::Line(_line) => todo!(),
        Geometry::LineString(line_string) => {
            if let Some(stroke) = line_stroke(properties)? {
                let points = line_string
                    .0
                    .iter()
                    .map(|p| pos2(p.x, p.y))
                    .collect::<Vec<_>>();
                shapes.push(Shape::line(points, stroke));
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
                    shapes.push(Shape::line(points, stroke));
                }
            }
        }
        Geometry::Polygon(_polygon) => todo!(),
        Geometry::MultiPoint(multi_point) => match kind(properties)?.as_str() {
            "neighbourhood" | "locality" => {
                if let Some(Value::String(name)) = properties.get("name") {
                    for point in multi_point.0.iter() {
                        shapes.push(text(pos2(point.x(), point.y()), name.clone(), egui_ctx));
                    }
                }
            }
            _ => {
                return Err(Error::UnsupportedFeatureKind(properties.clone()));
            }
        },
        Geometry::MultiPolygon(multi_polygon) => {
            if let Some(fill) = polygon_fill(properties)? {
                for polygon in multi_polygon.iter() {
                    let points = polygon
                        .exterior()
                        .0
                        .iter()
                        .map(|p| pos2(p.x, p.y))
                        .collect::<Vec<_>>();
                    shapes.extend(arbitrary_polygon(&points, fill));
                }
            }
        }
        Geometry::GeometryCollection(_geometry_collection) => todo!(),
        Geometry::Rect(_rect) => todo!(),
        Geometry::Triangle(_triangle) => todo!(),
    }
    Ok(())
}

fn text(pos: Pos2, text: String, ctx: &egui::Context) -> Shape {
    ctx.fonts_mut(|fonts| {
        Shape::text(
            fonts,
            pos,
            egui::Align2::CENTER_CENTER,
            text,
            FontId::proportional(80.0),
            Color32::from_gray(200),
        )
    })
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

fn polygon_fill(properties: &HashMap<String, Value>) -> Result<Option<Color32>, Error> {
    Ok(match kind(properties)?.as_str() {
        "water" | "fountain" | "swimming_pool" | "basin" | "lake" | "ditch" | "ocean" => {
            Some(WATER_COLOR)
        }
        "grass" | "garden" | "playground" | "zoo" | "park" | "forest" | "wood"
        | "village_green" | "scrub" | "grassland" | "allotments" | "pitch" | "farmland"
        | "dog_park" | "meadow" | "wetland" | "cemetery" | "golf_course" | "nature_reserve" => None,
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
        "path" => Some(Stroke::new(3.0, Color32::from_rgb(94, 62, 32))),
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
fn arbitrary_polygon(points: &[Pos2], fill: Color32) -> Vec<Shape> {
    let mut shapes = Vec::new();
    let mut triangles = Vec::<usize>::new();
    let mut earcut = earcut::Earcut::new();
    earcut.earcut(points.iter().map(|p| [p.x, p.y]), &[], &mut triangles);

    for triangle_indices in triangles.chunks(3) {
        let triangle = [
            points[triangle_indices[0]],
            points[triangle_indices[1]],
            points[triangle_indices[2]],
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
