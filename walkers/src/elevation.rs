use std::num::NonZeroUsize;

use lru::LruCache;

use crate::{
    Position, SourceData, TileData, TileId, TiledSource,
    tiles::heights::{Grid, Heights},
};

/// How many decoded tiles are kept. A few, so that reading around a corner where tiles meet does
/// not decode them over and over.
const CACHE_SIZE: NonZeroUsize = NonZeroUsize::MIN.saturating_add(3);

/// Elevation of the ground anywhere, read from a [`TiledSource`] made `with_elevation`, such as an
/// [`crate::HttpSource`] of [`crate::sources::Mapterhorn`].
///
/// ```no_run
/// # use walkers::{DemEncoding, Elevation, HttpOptions, HttpSource, lon_lat, sources::Mapterhorn};
/// # let ctx = egui::Context::default();
/// let terrain = HttpSource::new(Mapterhorn, HttpOptions::default(), ctx)
///     .with_elevation(DemEncoding::Terrarium);
/// let mut elevation = Elevation::new(terrain);
///
/// // Each frame, until the tile is fetched.
/// let sniezka = elevation.at(lon_lat(15.7400, 50.7360));
/// ```
pub struct Elevation<S> {
    source: S,

    /// `None` for tiles which are not elevation, or failed to be decoded.
    decoded: LruCache<TileId, Option<Grid>>,
}

impl<S: TiledSource> Elevation<S> {
    pub fn new(source: S) -> Self {
        Self {
            source,
            decoded: LruCache::new(CACHE_SIZE),
        }
    }

    /// In meters, at `position`, read from the deepest zoom level which the source has a tile of
    /// there, and interpolated between its pixels.
    ///
    /// `None` while that tile is being fetched, and where the source has no elevation. Fetching
    /// goes on in the background, and the map is repainted once the tile is there, so asking every
    /// frame is enough.
    pub fn at(&mut self, position: Position) -> Option<f32> {
        let mut zoom = self.source.max_zoom();

        loop {
            let (tile_id, within) = TileId::at(position, zoom);

            if let Some(grid) = self.decoded.get(&tile_id) {
                return grid.as_ref().map(|grid| sample(grid, within));
            }

            match self.source.data(tile_id) {
                TileData::Ready(data) => {
                    let grid = decode(&data);
                    let elevation = grid.as_ref().map(|grid| sample(grid, within));
                    self.decoded.put(tile_id, grid);
                    return elevation;
                }
                TileData::Loading => return None,
                // Where the source has no such tile, the one of the zoom level below covers it.
                TileData::Missing => zoom = zoom.checked_sub(1)?,
            }
        }
    }
}

fn decode(data: &SourceData) -> Option<Grid> {
    match data {
        SourceData::Elevation { data, encoding } => Heights::decode(data, *encoding)
            .inspect_err(|err| log::warn!("Failed to decode elevation: {err}"))
            .ok()
            .and_then(|heights| Grid::new(&heights)),
        _ => {
            log::warn!("Tiles of the source are not elevation. Is it made `with_elevation`?");
            None
        }
    }
}

/// At `within` the tile, from 0 at its top left corner to 1 at its bottom right.
fn sample(grid: &Grid, within: egui::Vec2) -> f32 {
    let width = grid.xs().last().copied().unwrap_or_default();
    let height = grid.ys().last().copied().unwrap_or_default();
    grid.sample(within.x * width, within.y * height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DemEncoding, lon_lat, sources::Attribution, tiles::heights::tests::tilted};
    use approx::assert_relative_eq;
    use std::{cell::Cell, rc::Rc, sync::Arc};

    /// The same Terrarium tile wherever there is one, which is from zoom 0 up to `deepest`.
    struct Terrain {
        png: bytes::Bytes,
        max_zoom: u8,
        deepest: u8,
        loading: bool,
        elevation: bool,

        /// How many times the data was asked for.
        asked: Rc<Cell<usize>>,
    }

    impl Terrain {
        fn new() -> Self {
            let mut png = Vec::new();
            tilted()
                .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                .unwrap();
            Self {
                png: png.into(),
                max_zoom: 0,
                deepest: 0,
                loading: false,
                elevation: true,
                asked: Rc::default(),
            }
        }
    }

    impl TiledSource for Terrain {
        fn data(&mut self, tile_id: TileId) -> TileData {
            self.asked.set(self.asked.get() + 1);
            if self.loading {
                TileData::Loading
            } else if tile_id.zoom > self.deepest {
                TileData::Missing
            } else {
                let data = self.png.clone();
                TileData::Ready(Arc::new(if self.elevation {
                    SourceData::Elevation {
                        data,
                        encoding: DemEncoding::Terrarium,
                    }
                } else {
                    SourceData::Encoded(data)
                }))
            }
        }

        fn attribution(&self) -> Attribution {
            Attribution {
                text: "",
                url: "",
                logo_light: None,
                logo_dark: None,
            }
        }

        fn max_zoom(&self) -> u8 {
            self.max_zoom
        }
    }

    /// At zoom 0, where the tile is the whole world, its middle, between the four middle pixels.
    const MIDDLE: f32 = 1000.0 + 10.0 * 7.5 + 100.0 * 7.5;

    #[test]
    fn elevation_is_read_and_interpolated_between_pixels() {
        let mut elevation = Elevation::new(Terrain::new());

        assert_relative_eq!(elevation.at(lon_lat(0.0, 0.0)).unwrap(), MIDDLE);

        // A quarter of a pixel east, out of 16 across 360 degrees.
        assert_relative_eq!(
            elevation.at(lon_lat(360.0 / 64.0, 0.0)).unwrap(),
            MIDDLE + 2.5,
            epsilon = 1e-3
        );
    }

    #[test]
    fn it_is_none_while_the_tile_is_being_fetched() {
        let mut elevation = Elevation::new(Terrain {
            loading: true,
            ..Terrain::new()
        });
        assert_eq!(elevation.at(lon_lat(0.0, 0.0)), None);
    }

    /// Most of the world has elevation only up to some zoom level below the max one of the source.
    #[test]
    fn missing_tile_is_read_from_a_lower_zoom_level() {
        let terrain = Terrain {
            max_zoom: 5,
            deepest: 2,
            ..Terrain::new()
        };
        let asked = terrain.asked.clone();
        let mut elevation = Elevation::new(terrain);

        // In the middle of the zoom 2 tile, as the one at zoom 0 was.
        let (tile_id, within) = TileId::at(lon_lat(45.0, 0.0), 2);
        assert_eq!(tile_id.zoom, 2);
        assert_eq!(within, egui::Vec2::new(0.5, 0.0));

        assert_relative_eq!(
            elevation.at(lon_lat(45.0, 0.0)).unwrap(),
            MIDDLE - 750.0 - 50.0
        );
        assert_eq!(asked.get(), 4);
    }

    #[test]
    fn tile_is_decoded_once() {
        let terrain = Terrain::new();
        let asked = terrain.asked.clone();
        let mut elevation = Elevation::new(terrain);

        elevation.at(lon_lat(0.0, 0.0));
        elevation.at(lon_lat(1.0, 1.0));
        assert_eq!(asked.get(), 1);
    }

    #[test]
    fn it_is_none_where_tiles_are_not_elevation() {
        let mut elevation = Elevation::new(Terrain {
            elevation: false,
            ..Terrain::new()
        });
        assert_eq!(elevation.at(lon_lat(0.0, 0.0)), None);
    }
}
