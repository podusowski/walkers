use egui::Ui;
use geo::MapCoords;
use geo::geometry::Coord;
use geojson::{Feature, GeoJson, JsonObject};
use log::warn;
use rstar::{AABB, RTree, RTreeObject};
use walkers::{Context, Layer, Position, Projector, Style, render_line};

struct IndexedFeature {
    properties: JsonObject,
    geometry: walkers::Geometry<f32>,
    envelope: AABB<[f64; 2]>,
}

impl RTreeObject for IndexedFeature {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

pub struct GeoJsonLayer {
    rtree: RTree<IndexedFeature>,
    style: Style,
}

impl GeoJsonLayer {
    pub fn new(geojson: GeoJson, style: Style) -> Self {
        let mut indexed = Vec::new();

        visit_features(&geojson, |feature| {
            if let Some(geom) = &feature.geometry {
                if let Ok(geometry) = walkers::Geometry::<f32>::try_from(geom.clone()) {
                    let envelope = compute_envelope(&geometry);
                    indexed.push(IndexedFeature {
                        properties: feature.properties.clone().unwrap_or_default(),
                        geometry,
                        envelope,
                    });
                }
            }
        });

        Self {
            rtree: RTree::bulk_load(indexed),
            style,
        }
    }

    pub fn render(&self, ui: &mut Ui, projector: &Projector, zoom: u8) {
        let viewport = viewport_envelope(projector, ui.clip_rect());

        let mut shapes = Vec::new();

        for layer in &self.style.layers {
            match layer {
                Layer::Line { paint, .. } => {
                    for entry in self.rtree.locate_in_envelope_intersecting(&viewport) {
                        let properties = entry.properties.clone().into_iter().collect();

                        let projected = project_geometry(&entry.geometry, projector);

                        let _ = render_line(
                            &projected,
                            &Context::new("geometry_type/TODO".to_string(), properties, zoom),
                            &mut shapes,
                            paint,
                        );
                    }
                }
                other => {
                    warn!("Unsupported style layer: {other:?}");
                }
            }
        }

        let painter = ui.painter();
        for shape in shapes {
            match shape {
                walkers::ShapeOrText::Shape(shape) => {
                    painter.add(shape);
                }
                walkers::ShapeOrText::Text(_) => {
                    // Text rendering not yet supported for GeoJSON layers.
                }
            }
        }
    }
}

/// Compute the geographic bounding box of a geometry (coordinates are lon/lat).
fn compute_envelope(geometry: &walkers::Geometry<f32>) -> AABB<[f64; 2]> {
    use geo::CoordsIter;

    let mut min_lon = f64::MAX;
    let mut min_lat = f64::MAX;
    let mut max_lon = f64::MIN;
    let mut max_lat = f64::MIN;

    for coord in geometry.coords_iter() {
        let lon = coord.x as f64;
        let lat = coord.y as f64;
        min_lon = min_lon.min(lon);
        min_lat = min_lat.min(lat);
        max_lon = max_lon.max(lon);
        max_lat = max_lat.max(lat);
    }

    AABB::from_corners([min_lon, min_lat], [max_lon, max_lat])
}

/// Compute the geographic envelope of the current viewport by unprojecting its corners.
fn viewport_envelope(projector: &Projector, clip_rect: egui::Rect) -> AABB<[f64; 2]> {
    let top_left = projector.unproject(clip_rect.min.to_vec2());
    let bottom_right = projector.unproject(clip_rect.max.to_vec2());

    // Position is geo_types::Point where x() = longitude, y() = latitude.
    let min_lon = top_left.x().min(bottom_right.x());
    let max_lon = top_left.x().max(bottom_right.x());
    let min_lat = top_left.y().min(bottom_right.y());
    let max_lat = top_left.y().max(bottom_right.y());

    AABB::from_corners([min_lon, min_lat], [max_lon, max_lat])
}

fn project_geometry(
    geometry: &walkers::Geometry<f32>,
    projector: &Projector,
) -> walkers::Geometry<f32> {
    geometry.map_coords(|coord| {
        let position = Position::new(coord.x as f64, coord.y as f64);
        let projected = projector.project(position);
        Coord {
            x: projected.x,
            y: projected.y,
        }
    })
}

fn visit_features(geojson: &GeoJson, mut visitor: impl FnMut(&Feature)) {
    match geojson {
        GeoJson::Geometry(_) => warn!("Top-level Geometry is not supported"),
        GeoJson::Feature(feature) => visitor(feature),
        GeoJson::FeatureCollection(feature_collection) => {
            for feature in &feature_collection.features {
                visitor(feature);
            }
        }
    }
}
