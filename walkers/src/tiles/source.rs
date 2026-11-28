use std::sync::Arc;
#[cfg(feature = "vector")]
use std::{
    collections::HashMap,
    sync::{Mutex, PoisonError},
};

use bytes::Bytes;

use crate::{Stats, TileId, sources::Attribution};
#[cfg(feature = "vector")]
use crate::{Value, geo_types::Geometry};

/// What a [`TiledSource`] gives for a tile, or an untiled source for all of its data, before any
/// style is applied to it.
#[non_exhaustive]
pub enum SourceData {
    /// A raster image (PNG, JPEG or WebP), or a Mapbox Vector Tile if the `mvt` feature is
    /// enabled. Which one it is, is told by the data itself.
    Encoded(Bytes),

    /// A raster image whose pixels are elevation, packed as `encoding` says.
    #[cfg(feature = "vector")]
    Elevation { data: Bytes, encoding: DemEncoding },

    /// Features in longitude and latitude, grouped in named layers. Only a [`Source`] gives them.
    #[cfg(feature = "vector")]
    Features(Vec<FeatureLayer>),
}

/// Features which style layers draw by naming the layer as their `source-layer`.
#[cfg(feature = "vector")]
#[derive(Clone, Debug, PartialEq)]
pub struct FeatureLayer {
    pub name: String,

    /// In the order they are drawn, each over the ones before it.
    pub features: Vec<Feature>,
}

#[cfg(feature = "vector")]
impl FeatureLayer {
    pub fn new(name: impl Into<String>, features: Vec<Feature>) -> Self {
        Self {
            name: name.into(),
            features,
        }
    }
}

/// Geometry in longitude and latitude, with the properties which style layers' filters and
/// expressions read.
#[cfg(feature = "vector")]
#[derive(Clone, Debug, PartialEq)]
pub struct Feature {
    pub geometry: Geometry<f64>,
    pub properties: HashMap<String, Value>,
}

#[cfg(feature = "vector")]
impl Feature {
    pub fn new(geometry: impl Into<Geometry<f64>>) -> Self {
        Self {
            geometry: geometry.into(),
            properties: HashMap::new(),
        }
    }

    pub fn with_property(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.properties.insert(name.into(), value.into());
        self
    }
}

/// What a [`TiledSource`] has for a tile.
#[derive(Clone)]
pub enum TileData {
    Ready(Arc<SourceData>),

    /// Being fetched, or about to be.
    Loading,

    /// There is no such tile, so the one of a lower zoom level covering it is drawn instead.
    Missing,
}

/// How elevation is packed into the pixels of a raster image.
///
/// <https://maplibre.org/maplibre-style-spec/sources/#encoding>
#[cfg(feature = "vector")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemEncoding {
    /// Mapbox Terrain-RGB.
    Mapbox,
    Terrarium,
}

#[cfg(feature = "vector")]
impl DemEncoding {
    /// Elevation, in meters, packed into a pixel's red, green, blue and alpha.
    pub fn decode(self, [r, g, b, _]: [u8; 4]) -> f32 {
        let (r, g, b) = (r as f32, g as f32, b as f32);
        match self {
            DemEncoding::Mapbox => (r * 65536.0 + g * 256.0 + b) * 0.1 - 10000.0,
            DemEncoding::Terrarium => r * 256.0 + g + b / 256.0 - 32768.0,
        }
    }
}

/// Source of tiles' data, such as a tile server or a PMTiles file. It knows nothing about how the
/// data is drawn, that is up to the style of [`crate::StyledSources`].
pub trait TiledSource {
    /// Data of that exact tile. Unless it is ready or missing, it is fetched in the background.
    fn data(&mut self, tile_id: TileId) -> TileData;

    fn attribution(&self) -> Attribution;

    /// Size of each tile, in pixels. Walkers works with 256px tiles internally, so this
    /// should be 256 multiplied or divided by a power of two, for example 128, 256 or 512.
    fn tile_size(&self) -> u32 {
        256
    }

    /// Tiles past it are drawn from the ones at this zoom level.
    fn max_zoom(&self) -> u8 {
        19
    }

    /// Tiles being loaded right now.
    fn stats(&self) -> Stats {
        Stats::default()
    }
}

/// Source of data which does not come in tiles, such as an app's own features. It knows nothing
/// about how the data is drawn, that is up to the style of [`crate::StyledSources`].
#[cfg(feature = "vector")]
pub trait Source {
    /// All of the data. A different `Arc` than the one before when the data has changed.
    fn data(&self) -> Arc<SourceData>;

    fn attribution(&self) -> Option<Attribution> {
        None
    }
}

/// Features kept in memory, and replaced when they change. Clones share them, so one can be
/// given to a [`crate::StyledSources`], and another kept for replacing them.
#[cfg(feature = "vector")]
#[derive(Clone)]
pub struct MemorySource {
    data: Arc<Mutex<Arc<SourceData>>>,
}

#[cfg(feature = "vector")]
impl MemorySource {
    pub fn new(layers: Vec<FeatureLayer>) -> Self {
        Self {
            data: Arc::new(Mutex::new(Arc::new(SourceData::Features(layers)))),
        }
    }

    /// Replaces all of the features. They are drawn from the next frame on, which this does not
    /// ask egui for.
    pub fn set(&self, layers: Vec<FeatureLayer>) {
        *self.data.lock().unwrap_or_else(PoisonError::into_inner) =
            Arc::new(SourceData::Features(layers));
    }
}

#[cfg(feature = "vector")]
impl Source for MemorySource {
    fn data(&self) -> Arc<SourceData> {
        self.data
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

#[cfg(all(test, feature = "vector"))]
mod tests {
    use super::*;

    #[test]
    fn elevation_is_unpacked_from_either_encoding() {
        assert_eq!(DemEncoding::Terrarium.decode([128, 0, 0, 255]), 0.0);
        assert_eq!(DemEncoding::Terrarium.decode([131, 232, 128, 255]), 1000.5);
        assert_eq!(DemEncoding::Mapbox.decode([1, 134, 160, 255]), 0.0);
        assert_eq!(DemEncoding::Mapbox.decode([1, 173, 176, 255]), 1000.0);
    }
}
