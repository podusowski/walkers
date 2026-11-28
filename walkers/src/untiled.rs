//! Data of an untiled [`Source`], styled in the map's pixels every frame.

use std::{ops::Range, sync::Arc};

use emath::{Rect, TSTransform};
use geo::{BoundingRect, MapCoords};
use rstar::{
    AABB, RTree,
    primitives::{GeomWithData, Rectangle},
};

use crate::{
    Canvas, Drawables, Position, Projector, Source, SourceData, Style,
    expression::Context,
    geo_types::Coord,
    render,
    sources::Attribution,
    tiles::{
        LABEL_MARGIN,
        apply_style::{DecodedLayer, apply_style_to_features, draws_from},
        grid,
    },
};

/// Untiled data is styled at the zoom levels of 512px tiles, which styles are written for.
const STYLED_AS_TILE_SIZE: u32 = 512;

/// A [`Source`], with its features looked up by where they are.
pub(crate) struct Untiled {
    source: Box<dyn Source>,
    indexed: Option<Indexed>,
}

/// Data of a source, and where each of its features is.
struct Indexed {
    data: Arc<SourceData>,
    bounds: RTree<Bounds>,
}

/// Bounds of a feature in longitude and latitude, with the index of its layer and its own within
/// that layer.
type Bounds = GeomWithData<Rectangle<[f64; 2]>, (usize, usize)>;

impl Untiled {
    pub fn new(source: Box<dyn Source>) -> Self {
        Self {
            source,
            indexed: None,
        }
    }

    pub fn attribution(&self) -> Option<Attribution> {
        self.source.attribution()
    }

    /// Draws `layers` of the style from the features on the map.
    pub fn draw(
        &mut self,
        canvas: &mut Canvas,
        projector: &Projector,
        style: &Style,
        layers: Range<usize>,
    ) {
        let Indexed { data, bounds } = self.indexed();
        let SourceData::Features(feature_layers) = &**data else {
            return;
        };

        let map_rect = projector.map_rect();
        let origin = map_rect.min.to_vec2();
        let offset = grid::zoom_offset(STYLED_AS_TILE_SIZE);
        let zoom = (projector.zoom().round() as i32 - offset).clamp(0, u8::MAX as i32) as u8;

        let needed: Vec<bool> = feature_layers
            .iter()
            .map(|layer| draws_from(style, layers.clone(), &layer.name))
            .collect();

        // Looked up by where they are, but drawn in their order.
        let mut found: Vec<(usize, usize)> = bounds
            .locate_in_envelope_intersecting(visible(projector, map_rect.expand(LABEL_MARGIN)))
            .map(|entry| entry.data)
            .filter(|(layer, _)| needed[*layer])
            .collect();
        found.sort_unstable();

        let mut decoded: Vec<DecodedLayer> = feature_layers
            .iter()
            .map(|layer| DecodedLayer {
                name: layer.name.clone(),
                features: Vec::new(),
            })
            .collect();

        for (layer, feature) in found {
            let feature = &feature_layers[layer].features[feature];
            let geometry = feature.geometry.map_coords(|coord| {
                let pixels = projector.project(Position::new(coord.x, coord.y)) - origin;
                Coord {
                    x: pixels.x,
                    y: pixels.y,
                }
            });
            let context = Context::new(
                render::geometry_type_to_str(&geometry).to_owned(),
                feature.properties.clone(),
                zoom,
            );
            decoded[layer].features.push((geometry, context));
        }

        let extent = map_rect.width().max(map_rect.height()).ceil() as u32;
        let (drawables, symbols) = apply_style_to_features(decoded, style, layers, zoom, extent);
        let transform = TSTransform::from_translation(origin);

        if !drawables.is_empty() {
            canvas.add_drawables(Drawables::merged(Arc::new(drawables)).with_transform(transform));
        }

        let z_order = canvas.z_order();
        for symbol in
            render::transformed_symbols(&symbols, transform, projector.zoom() - offset as f64)
        {
            canvas.add_symbol(z_order.combine(symbol.layer), symbol);
        }
    }

    /// The source's data, indexed again if it has changed.
    fn indexed(&mut self) -> &Indexed {
        let data = self.source.data();
        if !self
            .indexed
            .as_ref()
            .is_some_and(|indexed| Arc::ptr_eq(&indexed.data, &data))
        {
            self.indexed = None;
        }
        self.indexed.get_or_insert_with(|| Indexed::of(data))
    }
}

impl Indexed {
    fn of(data: Arc<SourceData>) -> Self {
        let mut entries = Vec::new();
        if let SourceData::Features(layers) = &*data {
            for (layer_index, layer) in layers.iter().enumerate() {
                for (feature_index, feature) in layer.features.iter().enumerate() {
                    if let Some(rect) = feature.geometry.bounding_rect() {
                        entries.push(GeomWithData::new(
                            Rectangle::from_corners(rect.min().into(), rect.max().into()),
                            (layer_index, feature_index),
                        ));
                    }
                }
            }
        }

        Self {
            data,
            bounds: RTree::bulk_load(entries),
        }
    }
}

/// Longitudes and latitudes within `rect` of the screen.
fn visible(projector: &Projector, rect: Rect) -> AABB<[f64; 2]> {
    let [a, b] = [rect.min, rect.max].map(|corner| projector.unproject(corner.to_vec2()));
    AABB::from_corners([a.x(), a.y()], [b.x(), b.y()])
}
