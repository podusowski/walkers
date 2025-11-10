use std::sync::{Arc, Mutex};

use egui::Context;
use futures::channel::mpsc::{Receiver, Sender, channel};
use lru::LruCache;

use crate::{
    HttpOptions, HttpStats, Texture, TileId, download::download_continuously, io::Runtime,
    sources::TileSource,
};

/// Asynchronously load tiles from different local and remote sources.

pub struct Loader {
    /// Tiles to be downloaded by the IO thread.
    pub request_tx: Sender<TileId>,

    /// Tiles that got downloaded and should be put in the cache.
    pub tile_rx: Receiver<(TileId, Texture)>,

    pub cache: LruCache<TileId, Option<Texture>>,
    pub http_stats: Arc<Mutex<HttpStats>>,

    #[allow(dead_code)] // Significant Drop
    runtime: Runtime,
}

impl Loader {
    /// Construct new [`Tiles`] with supplied [`HttpOptions`].
    pub fn with_options<S>(source: S, http_options: HttpOptions, egui_ctx: Context) -> Self
    where
        S: TileSource + Send + 'static,
    {
        let http_stats = Arc::new(Mutex::new(HttpStats { in_progress: 0 }));

        // This ensures that newer requests are prioritized.
        let channel_size = http_options.max_parallel_downloads.0;

        let (request_tx, request_rx) = channel(channel_size);
        let (tile_tx, tile_rx) = channel(channel_size);

        // This will run concurrently in a loop, handing downloads and talk with us via channels.
        let runtime = Runtime::new(download_continuously(
            source,
            http_options,
            http_stats.clone(),
            request_rx,
            tile_tx,
            egui_ctx,
        ));

        // Just arbitrary value which seemed right.
        #[allow(clippy::unwrap_used)]
        let cache_size = std::num::NonZeroUsize::new(256).unwrap();

        Self {
            cache: LruCache::new(cache_size),
            http_stats,
            request_tx,
            tile_rx,
            runtime,
        }
    }
}
