use bytes::Bytes;
use egui::Context;
use futures::{
    SinkExt, StreamExt,
    channel::mpsc::{Receiver, Sender, TryRecvError, channel},
};
use lru::LruCache;

use crate::{Tile, TileId, io::runtime::Runtime, tiles::TileError};

pub trait TileFactory {
    fn create_tile(&self, data: &Bytes, zoom: u8) -> Result<Tile, TileError>;
}

/// How many tiles can wait to be styled. Arbitrary, the rest is asked for again later.
const QUEUE_SIZE: usize = 8;

/// Turns fetched data into tiles in the background, and keeps them in a cache.
pub struct StylingIo {
    /// Data to be styled by the styling thread.
    request_tx: Sender<(TileId, Bytes)>,

    /// Tiles that got styled and should be put in the cache. `None` if styling failed.
    tile_rx: Receiver<(TileId, Option<Tile>)>,

    /// `None` while being styled, and forever if styling failed.
    pub cache: LruCache<TileId, Option<Tile>>,

    /// Sent to be styled, but not back yet.
    in_progress: usize,

    #[allow(dead_code)] // Significant Drop
    runtime: Runtime,
}

impl StylingIo {
    pub fn new(tile_factory: impl TileFactory + Send + Sync + 'static, egui_ctx: Context) -> Self {
        let (request_tx, request_rx) = channel(QUEUE_SIZE);
        let (tile_tx, tile_rx) = channel(QUEUE_SIZE);

        let runtime = Runtime::new(style_continuously(
            tile_factory,
            request_rx,
            tile_tx,
            egui_ctx,
        ));

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

    /// Takes a single styled tile from the styling thread and puts it in the cache.
    pub fn put_single_styled_tile_in_cache(&mut self) {
        // This is called every frame, so take just one at the time.
        match self.tile_rx.try_recv() {
            Ok((tile_id, tile)) => {
                self.cache.put(tile_id, tile);
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

    /// Whether the tile is styled, being styled, or failed to be.
    pub fn knows(&self, tile_id: TileId) -> bool {
        self.cache.contains(&tile_id)
    }

    /// Style `data` into the tile, unless there are too many waiting already, in which case it
    /// has to be asked for again.
    pub fn style(&mut self, tile_id: TileId, data: Bytes) {
        match self.request_tx.try_send((tile_id, data)) {
            Ok(()) => {
                self.cache.put(tile_id, None);
                self.in_progress += 1;
            }
            Err(err) if err.is_full() => {
                log::trace!("Styling queue is full.");
            }
            Err(err) => {
                panic!("Failed to send {tile_id:?} to be styled: {err}");
            }
        }
    }

    /// How many tiles are being styled right now.
    pub fn in_progress(&self) -> usize {
        self.in_progress
    }
}

/// Style the data coming through `request_rx`, until the main thread goes away.
async fn style_continuously(
    tile_factory: impl TileFactory,
    mut request_rx: Receiver<(TileId, Bytes)>,
    mut tile_tx: Sender<(TileId, Option<Tile>)>,
    egui_ctx: Context,
) {
    while let Some((tile_id, data)) = request_rx.next().await {
        let tile = tile_factory
            .create_tile(&data, tile_id.zoom)
            // It would probably be more consistent to push it to the caller, but it's not
            // that important right now.
            .inspect_err(|err| log::warn!("{err}"))
            .ok();

        // Failures too, so that the main thread knows they are no longer in progress.
        if tile_tx.send((tile_id, tile)).await.is_err() {
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

    struct Failing;

    impl TileFactory for Failing {
        fn create_tile(&self, _data: &Bytes, _zoom: u8) -> Result<Tile, TileError> {
            Err(TileError::Empty)
        }
    }

    /// Data which cannot be styled is done with too, so it must not be reported as loading.
    #[test]
    fn tile_which_fails_to_be_styled_is_no_longer_in_progress() {
        let mut styling = StylingIo::new(Failing, Context::default());
        let tile_id = TileId {
            x: 0,
            y: 0,
            zoom: 0,
        };

        styling.style(tile_id, Bytes::from_static(b"garbage"));
        assert_eq!(styling.in_progress(), 1);

        let deadline = Instant::now() + Duration::from_secs(5);
        while styling.in_progress() > 0 {
            assert!(Instant::now() < deadline, "still in progress");
            styling.put_single_styled_tile_in_cache();
            std::thread::sleep(Duration::from_millis(10));
        }

        // And it is not styled again.
        assert!(styling.knows(tile_id));
    }
}
