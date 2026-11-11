use std::{ops::Range, sync::Arc};

use egui::Context;
use futures::{
    SinkExt, StreamExt,
    channel::mpsc::{Receiver, Sender, TryRecvError, channel},
};
use lru::LruCache;

use super::{apply_style::apply_style, interpolate_from_lower_zoom};
use crate::{
    SourceData, Style, Tile, TileData, TileId, TilePiece, TiledSource, io::runtime::Runtime,
};

/// How many tiles can wait to be styled. Arbitrary, the rest is asked for again later.
const QUEUE_SIZE: usize = 8;

/// A tile, as some layers of the style make it of the data of a source.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct Key {
    /// Index of the source.
    source: usize,
    layers: Range<usize>,
    tile_id: TileId,
}

struct Request {
    key: Key,
    data: Arc<SourceData>,
    tile_size: u32,
}

/// What a style makes of sources' tiles, styled in the background and kept in a cache.
pub(crate) struct Styling {
    /// Data to be styled by the styling thread.
    request_tx: Sender<Request>,

    /// Tiles that got styled and should be put in the cache. `None` if styling failed.
    tile_rx: Receiver<(Key, Option<Tile>)>,

    /// `None` while being styled, and forever if styling failed.
    cache: LruCache<Key, Option<Tile>>,

    /// Sent to be styled, but not back yet.
    in_progress: usize,

    #[allow(dead_code)] // Significant Drop
    runtime: Runtime,
}

impl Styling {
    pub(crate) fn new(style: Arc<Style>, egui_ctx: Context) -> Self {
        let (request_tx, request_rx) = channel(QUEUE_SIZE);
        let (tile_tx, tile_rx) = channel(QUEUE_SIZE);

        let runtime = Runtime::new(style_continuously(style, request_rx, tile_tx, egui_ctx));

        // Just arbitrary value which seemed right.
        #[allow(clippy::unwrap_used)]
        let cache_size = std::num::NonZeroUsize::new(256).unwrap();

        Self {
            request_tx,
            tile_rx,
            cache: LruCache::new(cache_size),
            in_progress: 0,
            runtime,
        }
    }

    /// That tile, as `layers` make it of the data of `source`, the source at `index`. If it is
    /// not styled yet, the part of the nearest zoomed-out one covering the same ground. Styles
    /// the tile once `source` has its data.
    pub(crate) fn at(
        &mut self,
        index: usize,
        source: &mut dyn TiledSource,
        layers: Range<usize>,
        tile_id: TileId,
    ) -> Option<TilePiece> {
        self.put_single_styled_tile_in_cache();

        if !tile_id.valid() {
            return None;
        }

        let key = |tile_id| Key {
            source: index,
            layers: layers.clone(),
            tile_id,
        };

        let mut tile_id_to_style = if tile_id.zoom > source.max_zoom() {
            interpolate_from_lower_zoom(tile_id, source.max_zoom()).0
        } else {
            tile_id
        };

        // Where the source has no such tile, the one of the zoom level below stands in.
        while !self.cache.contains(&key(tile_id_to_style)) {
            match source.data(tile_id_to_style) {
                TileData::Ready(data) => {
                    self.style(key(tile_id_to_style), data, source.tile_size());
                    break;
                }
                TileData::Loading => break,
                TileData::Missing => match tile_id_to_style.zoom.checked_sub(1) {
                    Some(zoom) => {
                        tile_id_to_style = interpolate_from_lower_zoom(tile_id_to_style, zoom).0;
                    }
                    None => break,
                },
            }
        }

        self.best_available(key(tile_id), tile_id)
    }

    /// How many tiles are being styled right now.
    pub(crate) fn in_progress(&self) -> usize {
        self.in_progress
    }

    /// Takes a single styled tile from the styling thread and puts it in the cache.
    fn put_single_styled_tile_in_cache(&mut self) {
        // This is called every frame, so take just one at the time.
        match self.tile_rx.try_recv() {
            Ok((key, tile)) => {
                self.cache.put(key, tile);
                self.in_progress -= 1;
            }
            Err(TryRecvError::Empty) => {
                // No new tile was styled, just ignore.
            }
            Err(TryRecvError::Closed) => {
                log::error!("Styling thread is dead");
            }
        }
    }

    /// Style `data` into the tile, unless there are too many waiting already, in which case it
    /// has to be asked for again.
    fn style(&mut self, key: Key, data: Arc<SourceData>, tile_size: u32) {
        let request = Request {
            key: key.clone(),
            data,
            tile_size,
        };

        match self.request_tx.try_send(request) {
            Ok(()) => {
                self.cache.put(key, None);
                self.in_progress += 1;
            }
            Err(err) if err.is_full() => {
                log::trace!("Styling queue is full.");
            }
            Err(err) => {
                panic!("Failed to send {key:?} to be styled: {err}");
            }
        }
    }

    /// That tile, `key` is of, if it is cached, otherwise the part of the nearest zoomed-out one
    /// covering the same ground.
    fn best_available(&mut self, key: Key, tile_id: TileId) -> Option<TilePiece> {
        let mut zoom_candidate = tile_id.zoom;

        loop {
            let (zoomed_tile_id, uv) = interpolate_from_lower_zoom(tile_id, zoom_candidate);
            let zoomed_key = Key {
                tile_id: zoomed_tile_id,
                ..key.clone()
            };

            if let Some(Some(tile)) = self.cache.get(&zoomed_key) {
                break Some(TilePiece {
                    tile: tile.clone(),
                    uv,
                });
            }

            // Keep zooming out until we find a donor or there is no more zoom levels.
            zoom_candidate = zoom_candidate.checked_sub(1)?;
        }
    }
}

/// Style the data coming through `request_rx`, until the main thread goes away.
async fn style_continuously(
    style: Arc<Style>,
    mut request_rx: Receiver<Request>,
    mut tile_tx: Sender<(Key, Option<Tile>)>,
    egui_ctx: Context,
) {
    while let Some(Request {
        key,
        data,
        tile_size,
    }) = request_rx.next().await
    {
        let tile = apply_style(&data, &style, key.layers.clone(), key.tile_id, tile_size)
            // It would probably be more consistent to push it to the caller, but it's not that
            // important right now.
            .inspect_err(|err| log::warn!("{err}"))
            .ok()
            .map(|styled| Tile::from_styled(styled, &egui_ctx));

        // Failures too, so that the main thread knows they are no longer in progress.
        if tile_tx.send((key, tile)).await.is_err() {
            break;
        }
        egui_ctx.request_repaint();
    }

    log::debug!("Tile styling loop finished.");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// Only its tiles at zoom 0 exist, and they are all the same raster image.
    struct OnlyZoomZero {
        png: bytes::Bytes,
    }

    impl OnlyZoomZero {
        fn new() -> Self {
            let mut png = Vec::new();
            image::RgbaImage::new(1, 1)
                .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                .unwrap();
            Self { png: png.into() }
        }
    }

    impl TiledSource for OnlyZoomZero {
        fn data(&mut self, tile_id: TileId) -> TileData {
            if tile_id.zoom == 0 {
                TileData::Ready(Arc::new(SourceData::Encoded(self.png.clone())))
            } else {
                TileData::Missing
            }
        }

        fn attribution(&self) -> crate::sources::Attribution {
            crate::sources::Attribution {
                text: "",
                url: "",
                logo_light: None,
                logo_dark: None,
            }
        }
    }

    /// Like elevation tiles past the zoom levels a region has, so that the map keeps drawing
    /// those of the region's highest one.
    #[test]
    fn missing_tile_is_drawn_from_the_one_of_a_lower_zoom_level() {
        let mut styling = Styling::new(Arc::new(Style::default()), Context::default());
        let mut source = OnlyZoomZero::new();
        let tile_id = TileId {
            x: 3,
            y: 2,
            zoom: 2,
        };

        let deadline = Instant::now() + Duration::from_secs(5);
        let piece = loop {
            if let Some(piece) = styling.at(0, &mut source, 0..1, tile_id) {
                break piece;
            }
            assert!(Instant::now() < deadline, "never drawn");
            std::thread::sleep(Duration::from_millis(10));
        };

        // Where it lies in the tile of zoom 0.
        assert_eq!(piece.uv.min, egui::pos2(0.75, 0.5));
    }

    /// Data which cannot be styled is done with too, so it must not be reported as loading.
    #[test]
    fn tile_which_fails_to_be_styled_is_no_longer_in_progress() {
        let mut styling = Styling::new(Arc::new(Style::default()), Context::default());
        let key = Key {
            source: 0,
            layers: 0..0,
            tile_id: TileId {
                x: 0,
                y: 0,
                zoom: 0,
            },
        };

        // Empty data never makes a tile.
        styling.style(
            key.clone(),
            Arc::new(SourceData::Encoded(bytes::Bytes::new())),
            256,
        );
        assert_eq!(styling.in_progress(), 1);

        let deadline = Instant::now() + Duration::from_secs(5);
        while styling.in_progress() > 0 {
            assert!(Instant::now() < deadline, "still in progress");
            styling.put_single_styled_tile_in_cache();
            std::thread::sleep(Duration::from_millis(10));
        }

        // And it is not styled again.
        assert!(styling.cache.contains(&key));
    }
}
