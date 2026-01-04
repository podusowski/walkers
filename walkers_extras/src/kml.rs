use std::os::unix::raw::pthread_t;
use std::str::FromStr;
use std::sync::Arc;

use egui::{self, Color32, Response, Shape, Stroke, Ui};
use kml::{Kml, KmlDocument};
use lyon_path::geom::Point;
use lyon_tessellation::math::point;
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use thiserror::Error;
use walkers::{MapMemory, Plugin, Position, Projector, lon_lat, tessellate_polygon};

/// Geometry variants supported by the KML parser.
#[derive(Debug, Clone, PartialEq)]
pub enum KmlGeometry {
    Point(Position),
    LineString(Vec<Position>),
    Polygon {
        exterior: Vec<Position>,
        holes: Vec<Vec<Position>>,
    },
}

/// Basic styling information extracted from KML.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KmlStyle {
    pub stroke_color: Option<Color32>,
    pub stroke_width: Option<f32>,
    pub fill_color: Option<Color32>,
    pub fill: Option<bool>,
    pub outline: Option<bool>,
    pub icon_color: Option<Color32>,
    pub icon_scale: Option<f32>,
}

/// Parsed KML feature (Placemark).
#[derive(Debug, Clone, PartialEq)]
pub struct KmlFeature {
    pub name: Option<String>,
    pub description: Option<String>,
    pub geometries: Vec<KmlGeometry>,
    pub style: KmlStyle,
    pub style_url: Option<String>,
}

impl KmlFeature {
    pub fn new() -> Self {
        Self {
            name: None,
            description: None,
            geometries: Vec::new(),
            style: KmlStyle::default(),
            style_url: None,
        }
    }
}

impl Default for KmlFeature {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Error)]
pub enum KmlError {
    #[error("XML error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("Failed to parse integer: {0}")]
    ParseInt(#[from] std::num::ParseIntError),
    #[error("Invalid UTF-8 sequence: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("Unexpected geometry context for coordinates")]
    UnexpectedCoordinatesContext,
    #[error("Polygon missing exterior ring")]
    PolygonMissingExterior,
    #[error("Invalid coordinate triple: {0}")]
    InvalidCoordinate(String),
    #[error("Failed to parse float: {0}")]
    ParseFloat(#[from] std::num::ParseFloatError),
    #[error("Unsupported empty geometry")]
    EmptyGeometry,
    #[error("Boolean parse error: {0}")]
    ParseBool(String),
}

#[derive(Default)]
struct PolygonBuilder {
    exterior: Option<Vec<Position>>,
    holes: Vec<Vec<Position>>,
}

/// Parse a KML string into a list of features.
pub fn parse_kml(input: &str) -> Result<Vec<KmlFeature>, KmlError> {
    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut features = Vec::new();
    let mut current_feature: Option<KmlFeature> = None;
    let mut polygon_stack: Vec<PolygonBuilder> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(start) => {
                let name = decode(&start);
                stack.push(name.clone());
                handle_start(&name, &mut current_feature, &mut polygon_stack, &start);
            }
            Event::Empty(start) => {
                let name = decode(&start);
                stack.push(name.clone());
                handle_start(&name, &mut current_feature, &mut polygon_stack, &start);
                handle_end(
                    &name,
                    &mut current_feature,
                    &mut polygon_stack,
                    &mut features,
                )?;
                stack.pop();
            }
            Event::End(end) => {
                let name = decode_end(&end);
                handle_end(
                    &name,
                    &mut current_feature,
                    &mut polygon_stack,
                    &mut features,
                )?;
                stack.pop();
            }
            Event::Text(text) => {
                let value = text.unescape()?.trim().to_owned();
                if value.is_empty() {
                    buf.clear();
                    continue;
                }
                handle_text(&stack, &value, &mut current_feature, &mut polygon_stack)?;
            }
            Event::CData(text) => {
                let bytes = text.into_inner();
                let value = std::str::from_utf8(&bytes)?.trim().to_owned();
                if value.is_empty() {
                    buf.clear();
                    continue;
                }
                handle_text(&stack, &value, &mut current_feature, &mut polygon_stack)?;
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(features)
}

#[allow(clippy::ptr_arg)]
fn handle_start(
    name: &str,
    current_feature: &mut Option<KmlFeature>,
    polygon_stack: &mut Vec<PolygonBuilder>,
    _start: &BytesStart<'_>,
) {
    if name == "Placemark" && current_feature.is_none() {
        *current_feature = Some(KmlFeature::new());
    } else if name == "Polygon" {
        polygon_stack.push(PolygonBuilder::default());
    }
}

#[allow(clippy::ptr_arg)]
fn handle_end(
    name: &str,
    current_feature: &mut Option<KmlFeature>,
    polygon_stack: &mut Vec<PolygonBuilder>,
    features: &mut Vec<KmlFeature>,
) -> Result<(), KmlError> {
    match name {
        "Placemark" => {
            if let Some(feature) = current_feature.take() {
                if !feature.geometries.is_empty() {
                    features.push(feature);
                }
            }
        }
        "Polygon" => {
            if let Some(builder) = polygon_stack.pop() {
                let exterior = builder.exterior.ok_or(KmlError::PolygonMissingExterior)?;
                let geometry = KmlGeometry::Polygon {
                    exterior,
                    holes: builder.holes,
                };
                if let Some(feature) = current_feature.as_mut() {
                    feature.geometries.push(geometry);
                }
            }
        }
        _ => {}
    }
    Ok(())
}

#[allow(clippy::ptr_arg)]
fn handle_text(
    stack: &[String],
    value: &str,
    current_feature: &mut Option<KmlFeature>,
    polygon_stack: &mut Vec<PolygonBuilder>,
) -> Result<(), KmlError> {
    if let Some(feature) = current_feature.as_mut() {
        if stack_ends_with(stack, &["Placemark", "name"]) {
            feature.name = Some(value.to_owned());
        } else if stack_ends_with(stack, &["Placemark", "description"]) {
            feature.description = Some(value.to_owned());
        } else if stack_ends_with(stack, &["Placemark", "styleUrl"]) {
            feature.style_url = Some(value.to_owned());
        } else if stack_ends_with(stack, &["Placemark", "Style", "LineStyle", "color"]) {
            feature.style.stroke_color = Some(parse_kml_color(value)?);
        } else if stack_ends_with(stack, &["Placemark", "Style", "LineStyle", "width"]) {
            feature.style.stroke_width = Some(value.parse::<f32>()?);
        } else if stack_ends_with(stack, &["Placemark", "Style", "PolyStyle", "color"]) {
            feature.style.fill_color = Some(parse_kml_color(value)?);
        } else if stack_ends_with(stack, &["Placemark", "Style", "PolyStyle", "fill"]) {
            feature.style.fill = Some(parse_kml_bool(value)?);
        } else if stack_ends_with(stack, &["Placemark", "Style", "PolyStyle", "outline"]) {
            feature.style.outline = Some(parse_kml_bool(value)?);
        } else if stack_ends_with(stack, &["Placemark", "Style", "IconStyle", "color"]) {
            feature.style.icon_color = Some(parse_kml_color(value)?);
        } else if stack_ends_with(stack, &["Placemark", "Style", "IconStyle", "scale"]) {
            feature.style.icon_scale = Some(value.parse::<f32>()?);
        } else if stack.last().is_some_and(|s| s == "coordinates") {
            let coords = parse_coordinates(value)?;
            let geom_parent = stack
                .iter()
                .rev()
                .skip(1)
                .find(|name| matches!(name.as_str(), "Point" | "LineString" | "LinearRing"))
                .ok_or(KmlError::UnexpectedCoordinatesContext)?
                .clone();

            match geom_parent.as_str() {
                "Point" => {
                    let point = coords.into_iter().next().ok_or_else(|| {
                        KmlError::InvalidCoordinate("Point requires coordinates".into())
                    })?;
                    feature.geometries.push(KmlGeometry::Point(point));
                }
                "LineString" => {
                    if coords.is_empty() {
                        return Err(KmlError::EmptyGeometry);
                    }
                    feature.geometries.push(KmlGeometry::LineString(coords));
                }
                "LinearRing" => {
                    if let Some(polygon) = polygon_stack.last_mut() {
                        if stack.iter().any(|name| name == "outerBoundaryIs") {
                            polygon.exterior = Some(coords);
                        } else if stack.iter().any(|name| name == "innerBoundaryIs") {
                            polygon.holes.push(coords);
                        } else {
                            polygon.exterior = Some(coords);
                        }
                    }
                }
                _ => return Err(KmlError::UnexpectedCoordinatesContext),
            }
        }
    }

    Ok(())
}

fn stack_ends_with(stack: &[String], suffix: &[&str]) -> bool {
    if suffix.len() > stack.len() {
        return false;
    }
    stack[stack.len() - suffix.len()..]
        .iter()
        .zip(suffix.iter())
        .all(|(a, b)| a == b)
}

fn parse_coordinates(text: &str) -> Result<Vec<Position>, KmlError> {
    let mut positions = Vec::new();
    for token in text
        .split(|c: char| c.is_ascii_whitespace())
        .filter(|s| !s.is_empty())
    {
        let mut parts = token.split(',');
        let lon = parts
            .next()
            .ok_or_else(|| KmlError::InvalidCoordinate(token.to_string()))?
            .parse::<f64>()?;
        let lat = parts
            .next()
            .ok_or_else(|| KmlError::InvalidCoordinate(token.to_string()))?
            .parse::<f64>()?;
        positions.push(lon_lat(lon, lat));
    }
    Ok(positions)
}

fn parse_kml_color(text: &str) -> Result<Color32, KmlError> {
    let trimmed = text.trim();
    if trimmed.len() != 8 {
        return Err(KmlError::InvalidCoordinate(trimmed.to_string()));
    }
    let a = u8::from_str_radix(&trimmed[0..2], 16)?;
    let b = u8::from_str_radix(&trimmed[2..4], 16)?;
    let g = u8::from_str_radix(&trimmed[4..6], 16)?;
    let r = u8::from_str_radix(&trimmed[6..8], 16)?;
    Ok(Color32::from_rgba_unmultiplied(r, g, b, a))
}

fn parse_kml_bool(text: &str) -> Result<bool, KmlError> {
    match text.trim() {
        "1" | "true" | "True" | "TRUE" => Ok(true),
        "0" | "false" | "False" | "FALSE" => Ok(false),
        other => Err(KmlError::ParseBool(other.to_string())),
    }
}

fn decode(start: &BytesStart<'_>) -> String {
    String::from_utf8_lossy(start.name().as_ref()).into_owned()
}

fn decode_end(end: &quick_xml::events::BytesEnd<'_>) -> String {
    String::from_utf8_lossy(end.name().as_ref()).into_owned()
}

/// Default styling values applied when a KML feature does not provide its own style information.
#[derive(Debug, Clone, PartialEq)]
pub struct KmlVisualDefaults {
    pub point_radius: f32,
    pub point_color: Color32,
    pub line_color: Color32,
    pub line_width: f32,
    pub polygon_fill_color: Color32,
    pub polygon_outline_color: Color32,
    pub polygon_outline_width: f32,
    pub fill_tolerance: f32,
}

impl Default for KmlVisualDefaults {
    fn default() -> Self {
        Self {
            point_radius: 6.0,
            point_color: Color32::from_rgb(0x1f, 0x77, 0xb4),
            line_color: Color32::from_rgb(0xff, 0x7f, 0x0e),
            line_width: 2.0,
            polygon_fill_color: Color32::from_rgba_unmultiplied(0x2c, 0xa0, 0x2c, 96),
            polygon_outline_color: Color32::from_rgb(0x00, 0x61, 0x5c),
            polygon_outline_width: 1.5,
            fill_tolerance: 0.5,
        }
    }
}

#[derive(Clone)]
struct KmlLayerState {
    pub kml: kml::Kml,
    features: Vec<KmlFeature>,
    defaults: KmlVisualDefaults,
}

impl KmlLayerState {
    fn draw_geometry(
        &self,
        painter: &egui::Painter,
        response: &Response,
        projector: &Projector,
        geometry: &kml::types::Geometry,
    ) {
        println!("Drawing geometry: {:?}", geometry);
        match geometry {
            kml::types::Geometry::Point(point) => {
                let position = lon_lat(point.coord.x, point.coord.y);
                let (radius, color) = resolve_point_style(&KmlFeature::default(), &self.defaults);
                let screen = projector.project(position).to_pos2();
                painter.circle_filled(screen, radius, color);
            }
            kml::types::Geometry::LineString(line_string) => todo!(),
            kml::types::Geometry::LinearRing(linear_ring) => todo!(),
            kml::types::Geometry::Polygon(polygon) => (),
            kml::types::Geometry::MultiGeometry(multi_geometry) => {
                for geom in &multi_geometry.geometries {
                    self.draw_geometry(painter, response, projector, geom);
                }
            }
            kml::types::Geometry::Element(element) => todo!(),
            _ => todo!(),
        }
    }

    fn draw(&self, ui: &mut Ui, response: &Response, projector: &Projector, element: &kml::Kml) {
        let painter = ui.painter_at(response.rect);

        match element {
            kml::Kml::Placemark(placemark) => {
                println!("Drawing placemark: {:?}", placemark);
                for geometry in &placemark.geometry {
                    self.draw_geometry(&painter, response, projector, geometry);
                }
            }
            kml::Kml::Document { elements, .. } => {
                println!("Drawing document with {} elements", elements.len());
                for child in elements {
                    self.draw(ui, response, projector, child);
                }
            }
            kml::Kml::KmlDocument(KmlDocument { elements, .. }) => {
                println!("Drawing kml document with {} elements", elements.len());
                for child in elements {
                    self.draw(ui, response, projector, child);
                }
            }
            kml::Kml::Folder(folder) => {
                println!("Drawing folder with {} elements", folder.elements.len());
                for child in &folder.elements {
                    self.draw(ui, response, projector, child);
                }
            }
            _ => {
                println!("Skipping unsupported KML element: {:?}", element);
            }
        }

        //for feature in &self.features {
        //    for geometry in &feature.geometries {
        //        match geometry {
        //            KmlGeometry::Point(position) => {
        //                let (radius, color) = resolve_point_style(feature, &self.defaults);
        //                let screen = projector.project(*position).to_pos2();
        //                painter.circle_filled(screen, radius, color);
        //            }
        //            KmlGeometry::LineString(positions) => {
        //                if positions.len() < 2 {
        //                    continue;
        //                }
        //                let stroke = resolve_line_style(feature, &self.defaults);
        //                let mut points = Vec::with_capacity(positions.len());
        //                for position in positions {
        //                    points.push(projector.project(*position).to_pos2());
        //                }
        //                painter.add(Shape::line(points, stroke));
        //            }
        //            KmlGeometry::Polygon { exterior, holes } => {
        //                draw_polygon(
        //                    &painter,
        //                    projector,
        //                    feature,
        //                    exterior,
        //                    holes,
        //                    &self.defaults,
        //                );
        //            }
        //        }
        //    }
        //}
    }
}

/// Plugin that renders parsed KML features on top of a [`Map`](walkers::Map).
#[derive(Clone)]
pub struct KmlLayer {
    inner: Arc<KmlLayerState>,
}

impl KmlLayer {
    pub fn from_string(s: &str) -> Self {
        Self {
            inner: Arc::new(KmlLayerState {
                kml: kml::Kml::from_str(s).unwrap(),
                features: Vec::new(),
                defaults: KmlVisualDefaults::default(),
            }),
        }
    }

    pub fn with_defaults(mut self, defaults: KmlVisualDefaults) -> Self {
        let mut state = (*self.inner).clone();
        state.defaults = defaults;
        self.inner = Arc::new(state);
        self
    }

    pub fn features(&self) -> &[KmlFeature] {
        &self.inner.features
    }
}

impl Plugin for KmlLayer {
    fn run(
        self: Box<Self>,
        ui: &mut Ui,
        response: &Response,
        projector: &Projector,
        _map_memory: &MapMemory,
    ) {
        self.inner.draw(ui, response, projector, &self.inner.kml);
    }
}

fn resolve_point_style(feature: &KmlFeature, defaults: &KmlVisualDefaults) -> (f32, Color32) {
    let color = feature.style.icon_color.unwrap_or(defaults.point_color);
    let scale = feature.style.icon_scale.unwrap_or(1.0).max(0.1);
    let radius = defaults.point_radius * scale;
    (radius, color)
}

fn resolve_line_style(feature: &KmlFeature, defaults: &KmlVisualDefaults) -> Stroke {
    let color = feature.style.stroke_color.unwrap_or(defaults.line_color);
    let width = feature
        .style
        .stroke_width
        .unwrap_or(defaults.line_width)
        .max(0.1);
    Stroke::new(width, color)
}

fn resolve_polygon_style(
    feature: &KmlFeature,
    defaults: &KmlVisualDefaults,
) -> (Option<Color32>, Option<Stroke>) {
    let fill_allowed = feature.style.fill.unwrap_or(true);
    let outline_allowed = feature.style.outline.unwrap_or(true);

    let fill_color = if fill_allowed {
        feature
            .style
            .fill_color
            .or(Some(defaults.polygon_fill_color))
    } else {
        None
    };

    let outline = if outline_allowed {
        let stroke_color = feature
            .style
            .stroke_color
            .unwrap_or(defaults.polygon_outline_color);
        let stroke_width = feature
            .style
            .stroke_width
            .unwrap_or(defaults.polygon_outline_width)
            .max(0.1);
        Some(Stroke::new(stroke_width, stroke_color))
    } else {
        None
    };

    (fill_color, outline)
}

fn draw_polygon(
    painter: &egui::Painter,
    projector: &Projector,
    feature: &KmlFeature,
    exterior: &[Position],
    holes: &[Vec<Position>],
    defaults: &KmlVisualDefaults,
) {
    let (fill_color, outline_stroke) = resolve_polygon_style(feature, defaults);

    let Some(exterior_screen) = ring_to_screen_points(exterior, projector) else {
        return;
    };

    let mut hole_points: Vec<Vec<Point<f32>>> = Vec::with_capacity(holes.len());
    for hole in holes {
        if let Some(points) = ring_to_screen_points(hole, projector) {
            hole_points.push(points);
        }
    }

    if let Some(fill_color) = fill_color {
        if let Ok(mesh) = tessellate_polygon(&exterior_screen, &hole_points, fill_color) {
            painter.add(Shape::mesh(mesh));
        }
    }

    if let Some(stroke) = outline_stroke {
        painter.add(Shape::closed_line(
            exterior_screen
                .iter()
                .map(|p| egui::pos2(p.x, p.y))
                .collect(),
            stroke,
        ));
        for hole in &hole_points {
            painter.add(Shape::closed_line(
                hole.iter().map(|p| egui::pos2(p.x, p.y)).collect(),
                stroke,
            ));
        }
    }
}

fn ring_to_screen_points(ring: &[Position], projector: &Projector) -> Option<Vec<Point<f32>>> {
    if ring.len() < 3 {
        return None;
    }
    let mut points = Vec::with_capacity(ring.len());
    for (idx, position) in ring.iter().enumerate() {
        if idx + 1 == ring.len() && ring[0].x() == position.x() && ring[0].y() == position.y() {
            // Skip duplicate closing vertex.
            continue;
        }
        let p = projector.project(*position).to_pos2();
        points.push(point(p.x, p.y));
    }
    if points.len() < 3 { None } else { Some(points) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_point() {
        let doc = r#"
            <kml xmlns="http://www.opengis.net/kml/2.2">
                <Placemark>
                    <name>Test point</name>
                    <Point>
                        <coordinates>-122.0822035425683,37.42228990140251,0</coordinates>
                    </Point>
                </Placemark>
            </kml>
        "#;

        let features = parse_kml(doc).unwrap();
        assert_eq!(features.len(), 1);
        assert_eq!(features[0].name.as_deref(), Some("Test point"));
        assert!(matches!(features[0].geometries[0], KmlGeometry::Point(_)));
    }

    #[test]
    fn parse_linestring() {
        let doc = r#"
            <kml xmlns="http://www.opengis.net/kml/2.2">
                <Placemark>
                    <LineString>
                        <coordinates>
                            -122.0822035425683,37.42228990140251,0
                            -122.0850000000000,37.42200000000000,0
                        </coordinates>
                    </LineString>
                </Placemark>
            </kml>
        "#;

        let features = parse_kml(doc).unwrap();
        assert_eq!(features.len(), 1);
        assert!(matches!(
            features[0].geometries[0],
            KmlGeometry::LineString(ref pts) if pts.len() == 2
        ));
    }

    #[test]
    fn parse_polygon_with_hole() {
        let doc = r#"
            <kml xmlns="http://www.opengis.net/kml/2.2">
                <Placemark>
                    <Polygon>
                        <outerBoundaryIs>
                            <LinearRing>
                                <coordinates>
                                    0,0,0 10,0,0 10,10,0 0,10,0 0,0,0
                                </coordinates>
                            </LinearRing>
                        </outerBoundaryIs>
                        <innerBoundaryIs>
                            <LinearRing>
                                <coordinates>
                                    2,2,0 2,4,0 4,4,0 4,2,0 2,2,0
                                </coordinates>
                            </LinearRing>
                        </innerBoundaryIs>
                    </Polygon>
                </Placemark>
            </kml>
        "#;

        let features = parse_kml(doc).unwrap();
        assert_eq!(features.len(), 1);
        match &features[0].geometries[0] {
            KmlGeometry::Polygon { exterior, holes } => {
                assert_eq!(exterior.len(), 5);
                assert_eq!(holes.len(), 1);
            }
            _ => panic!("expected polygon"),
        }
    }
}
