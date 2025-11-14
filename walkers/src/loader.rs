use std::sync::{Arc, Mutex};

use egui::Context;
use futures::channel::mpsc::{Receiver, Sender, TrySendError, channel};
use lru::LruCache;

use crate::{
    HttpStats, Tile, TileId,
    download::{Fetch, download_continuously},
    io::Runtime,
};

/// Asynchronously load and cache tiles from different local and remote sources.
pub struct Loader {
    /// Tiles to be fetched by the IO thread.
    pub request_tx: Sender<TileId>,

    /// Tiles that got fetched and should be put in the cache.
    pub tile_rx: Receiver<(TileId, Tile)>,

    pub cache: LruCache<TileId, Option<Tile>>,
    pub stats: Arc<Mutex<HttpStats>>,

    #[allow(dead_code)] // Significant Drop
    runtime: Runtime,
}

impl Loader {
    pub fn new(fetch: impl Fetch + Send + Sync + 'static, egui_ctx: Context) -> Self {
        let stats = Arc::new(Mutex::new(HttpStats { in_progress: 0 }));

        // This ensures that newer requests are prioritized.
        let channel_size = fetch.max_concurrency();

        let (request_tx, request_rx) = channel(channel_size);
        let (tile_tx, tile_rx) = channel(channel_size);

        // This will run concurrently in a loop, handing downloads and talk with us via channels.
        let runtime = Runtime::new(download_continuously(
            fetch,
            stats.clone(),
            request_rx,
            tile_tx,
            egui_ctx,
        ));

        // Just arbitrary value which seemed right.
        #[allow(clippy::unwrap_used)]
        let cache_size = std::num::NonZeroUsize::new(256).unwrap();

        Self {
            cache: LruCache::new(cache_size),
            stats,
            request_tx,
            tile_rx,
            runtime,
        }
    }

    pub fn put_single_downloaded_tile_in_cache(&mut self) {
        // This is called every frame, so take just one at the time.
        match self.tile_rx.try_next() {
            Ok(Some((tile_id, tile))) => {
                self.cache.put(tile_id, Some(tile));
            }
            Err(_) => {
                // Just ignore. It means that no new tile was downloaded.
            }
            Ok(None) => {
                log::error!("IO thread is dead")
            }
        }
    }

    pub fn make_sure_is_downloaded(&mut self, tile_id: TileId) {
        match self.cache.try_get_or_insert(
            tile_id,
            || -> Result<Option<Tile>, TrySendError<TileId>> {
                self.request_tx.try_send(tile_id)?;
                log::trace!("Requested tile: {tile_id:?}");
                Ok(None)
            },
        ) {
            Ok(_) => {}
            Err(err) if err.is_full() => {
                // Trying to download too many tiles at once.
                log::trace!("Request queue is full.");
            }
            Err(err) => {
                panic!("Failed to send tile request for {tile_id:?}: {err}");
            }
        }
    }
}
