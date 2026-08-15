use std::collections::HashMap;

use egui::Ui;
use geo::MapCoords;
use geo::geometry::Coord;
use geojson::{Feature as GeoJsonFeature, GeoJson};
use log::warn;
use rstar::primitives::{GeomWithData, Rectangle};
use rstar::{AABB, RTree};
use walkers::{
    Context, Filter, Layer, Position, Projector, Style, place_texts, render_line, render_symbol,
};

struct Feature {
    geometry: walkers::Geometry<f32>,
    properties: HashMap<String, walkers::Value>,
}

pub struct GeoJsonLayer {
    /// R-tree indexing the bounding rectangles of all features.
    rtree: RTree<GeomWithData<Rectangle<[f64; 2]>, Feature>>,
    style: Style,
}

impl GeoJsonLayer {
    pub fn new(geojson: GeoJson, style: Style) -> Self {
        let mut indexed = Vec::new();

        visit_features(&geojson, |feature| {
            if let Some(geometry) = &feature.geometry
                && let Ok(geometry) = walkers::Geometry::<f32>::try_from(geometry.clone())
            {
                indexed.push(GeomWithData::new(
                    bounding_rect(&geometry),
                    Feature {
                        geometry,
                        properties: feature
                            .properties
                            .clone()
                            .unwrap_or_default()
                            .into_iter()
                            .collect(),
                    },
                ));
            }
        });

        Self {
            rtree: RTree::bulk_load(indexed),
            style,
        }
    }

    pub fn render(&self, ui: &mut Ui, projector: &Projector, zoom: u8) {
        let viewport = viewport(projector, ui.clip_rect());

        let mut shapes = Vec::new();
        let mut texts = Vec::new();

        for layer in &self.style.layers {
            match layer {
                Layer::Line { paint, filter, .. } => {
                    for (geometry, context) in self.features(viewport, filter.as_ref(), zoom) {
                        let projected = project_geometry(geometry, projector);
                        let _ = render_line(&projected, &context, &mut shapes, paint);
                    }
                }
                Layer::Symbol {
                    layout,
                    paint,
                    filter,
                    ..
                } => {
                    for (geometry, context) in self.features(viewport, filter.as_ref(), zoom) {
                        let projected = project_geometry(geometry, projector);
                        let _ = render_symbol(&projected, &context, &mut texts, layout, paint);
                    }
                }
                other => {
                    warn!("Unsupported style layer: {other:?}");
                }
            }
        }

        // Geometry first, then the labels on top of it.
        let texts = place_texts(texts, ui.ctx());
        let painter = ui.painter();
        painter.extend(shapes);
        painter.extend(texts);
    }

    /// Features in the viewport which the layer's filter lets through.
    fn features<'a>(
        &'a self,
        viewport: AABB<[f64; 2]>,
        filter: Option<&'a Filter>,
        zoom: u8,
    ) -> impl Iterator<Item = (&'a walkers::Geometry<f32>, Context)> + 'a {
        self.rtree
            .locate_in_envelope_intersecting(viewport)
            .filter_map(move |entry| {
                let context = Context::new(
                    "geometry_type/TODO".to_string(),
                    entry.data.properties.clone(),
                    zoom,
                );

                match filter {
                    Some(filter) if !filter.matches(&context) => None,
                    _ => Some((&entry.data.geometry, context)),
                }
            })
    }
}

/// Compute the geographic bounding rectangle of a geometry (coordinates are lon/lat).
fn bounding_rect(geometry: &walkers::Geometry<f32>) -> Rectangle<[f64; 2]> {
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

    Rectangle::from_corners([min_lon, min_lat], [max_lon, max_lat])
}

/// Compute the geographic envelope of the current viewport by unprojecting its corners.
fn viewport(projector: &Projector, clip_rect: egui::Rect) -> AABB<[f64; 2]> {
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
        let projected = projector.project(Position::new(coord.x as f64, coord.y as f64));
        Coord {
            x: projected.x,
            y: projected.y,
        }
    })
}

fn visit_features(geojson: &GeoJson, mut visitor: impl FnMut(&GeoJsonFeature)) {
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
