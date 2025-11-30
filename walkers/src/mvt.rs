//! Renderer for Mapbox Vector Tiles.

use std::collections::HashMap;

use egui::{
    Color32, Mesh, Pos2, Shape, Stroke,
    emath::TSTransform,
    epaint::{Vertex, WHITE_UV},
    pos2,
};
use geo_types::Geometry;
use log::warn;
use lyon_path::{
    Path, Polygon,
    geom::{Point, point},
};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, TessellationError, VertexBuffers,
};
use mvt_reader::feature::{Feature, Value};

use crate::style::{Layer, Paint, Style};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Decoding MVT failed: {0}.")]
    Mvt(String),
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
    #[error(transparent)]
    Tessellation(#[from] TessellationError),
}

/// Custom conversion because mvt_reader::error::Error is not Send.
impl From<mvt_reader::error::ParserError> for Error {
    fn from(err: mvt_reader::error::ParserError) -> Self {
        Error::Mvt(err.to_string())
    }
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

impl From<Mesh> for ShapeOrText {
    fn from(mesh: Mesh) -> Self {
        ShapeOrText::Shape(Shape::Mesh(mesh.into()))
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
pub fn render(data: &[u8], style: &Style) -> Result<Vec<ShapeOrText>, Error> {
    let data = mvt_reader::Reader::new(data.to_vec())?;
    let mut shapes = Vec::new();

    for layer in &style.layers {
        match layer {
            Layer::Background => continue,
            Layer::Fill {
                id,
                source_layer,
                filter,
                paint,
            } => {
                let Ok(layer_index) = find_layer(&data, &source_layer) else {
                    warn!("Source layer '{source_layer}' not found. Skipping.");
                    continue;
                };

                for feature in data.get_features(layer_index)? {
                    if !match_filter(&feature, filter) {
                        continue;
                    }

                    if let Err(err) = feature_into_shape(&feature, &mut shapes, paint) {
                        warn!("{err}");
                    }
                }
            }
            layer => {
                log::warn!("Unsupported layer type in style: {layer:?}");
                continue;
            }
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

fn match_filter(feature: &Feature, filter: &Option<crate::style::Filter>) -> bool {
    match (&feature.properties, filter) {
        (Some(properties), Some(filter)) => filter.matches(&properties),
        _ => true,
    }
}

fn feature_into_shape(
    feature: &Feature,
    shapes: &mut Vec<ShapeOrText>,
    paint: &Paint,
) -> Result<(), Error> {
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
                    let interiors = polygon
                        .interiors()
                        .iter()
                        .map(|hole| hole.0.iter().map(|p| pos2(p.x, p.y)).collect::<Vec<_>>())
                        .collect::<Vec<_>>();
                    shapes.push(tessellate_polygon(&points, &interiors, fill)?.into());
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
        "country" => Ok(32.0),
        "neighbourhood" | "locality" => Ok(16.0),
        "peak" | "water" | "forest" | "park" | "national_park" | "protected_area"
        | "nature_reserve" | "military" | "hospital" | "bus_station" | "train_station"
        | "aerodrome" => Ok(10.0),
        _ => Err(Error::UnsupportedFeatureKind(properties.clone())),
    }?;

    if let Some(Value::String(name)) = properties.get("name") {
        Ok(points
            .iter()
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
        "military" => Some(Color32::from_rgb(40, 0, 0)),
        "sand" | "beach" => Some(Color32::from_rgb(64, 64, 0)),
        "pedestrian" | "recreation_ground" | "railway" | "industrial" | "residential"
        | "commercial" | "protected_area" | "school" | "platform" | "kindergarten" | "cliff"
        | "university" | "hospital" | "college" | "aerodrome" | "airfield" | "earth"
        | "urban_area" | "other" => None,
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
        "ferry" => Some(Stroke::new(3.0, Color32::from_rgb(15, 51, 102))),
        "other" | "aerialway" | "cliff" => None,
        _ => {
            warn!("Unknown line kind: {properties:?}");
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

/// Egui cannot tessellate complex polygons, so we use lyon for that.
fn tessellate_polygon(
    exterior: &[Pos2],
    interiors: &[Vec<Pos2>],
    fill_color: Color32,
) -> Result<Mesh, TessellationError> {
    let mut builder = Path::builder();

    builder.add_polygon(Polygon {
        points: &lyon_points(exterior),
        closed: true,
    });

    for interior in interiors {
        builder.add_polygon(Polygon {
            points: &lyon_points(interior),
            closed: true,
        });
    }

    let mut buffers: VertexBuffers<Vertex, u32> = VertexBuffers::new();

    FillTessellator::new().tessellate_path(
        &builder.build(),
        &FillOptions::default(),
        &mut BuffersBuilder::new(&mut buffers, |vertex: FillVertex| {
            let pos = vertex.position();
            Vertex {
                pos: pos2(pos.x, pos.y),
                uv: WHITE_UV,
                color: fill_color,
            }
        }),
    )?;

    Ok(Mesh {
        indices: buffers.indices,
        vertices: buffers.vertices,
        ..Default::default()
    })
}

fn lyon_points(points: &[Pos2]) -> Vec<Point<f32>> {
    points.iter().map(|p| point(p.x, p.y)).collect()
}
