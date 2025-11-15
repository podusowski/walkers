/// Asynchronous fetching loop.
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use bytes::Bytes;
use egui::Context;
use futures::{
    SinkExt, StreamExt,
    channel::mpsc::{Receiver, Sender},
    future::{Either, select, select_all},
};

use crate::{
    Stats, TileId,
    tiles::{Tile, TileError},
};

pub use reqwest::header::HeaderValue;

/// Controls how [`crate::HttpTiles`] use the HTTP protocol, such as caching.
pub struct HttpOptions {
    /// Path to the directory to store the HTTP cache.
    ///
    /// Keep in mind that some providers (such as OpenStreetMap) require clients
    /// to respect the HTTP `Expires` header.
    /// <https://operations.osmfoundation.org/policies/tiles/>
    ///
    /// This option is ignored in WASM, as HTTP cache is controlled by the
    /// browser the app is running on.
    pub cache: Option<PathBuf>,

    /// User agent to be sent to the tile servers.
    ///
    /// This should be set only on native targets. The browser sets its own user agent on wasm
    /// targets, and trying to set a different one may upset some servers (e.g. MapBox)
    pub user_agent: Option<HeaderValue>,

    /// Maximum number of parallel downloads.
    ///
    /// Many services have rate limits, and exceeding them may result in throttling, bans, or
    /// degraded service. Use the default value when in doubt.
    pub max_parallel_downloads: MaxParallelDownloads,
}

impl Default for HttpOptions {
    fn default() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let user_agent = Some(HeaderValue::from_static(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION"),
        )));

        #[cfg(target_arch = "wasm32")]
        let user_agent = None;

        Self {
            cache: None,
            user_agent,
            max_parallel_downloads: MaxParallelDownloads::default(),
        }
    }
}

/// Maximum number of parallel downloads.
pub struct MaxParallelDownloads(pub usize);

impl Default for MaxParallelDownloads {
    /// Default number of parallel downloads. Following modern browsers' behavior.
    /// <https://stackoverflow.com/questions/985431/max-parallel-http-connections-in-a-browser>
    fn default() -> Self {
        Self(6)
    }
}

impl MaxParallelDownloads {
    /// Use custom value.
    ///
    /// Many services have rate limits, and exceeding them may result in throttling, bans, or
    /// degraded service. You are **strongly encouraged** to check the Terms of Use of the
    /// particular provider you are using.
    pub fn value_manually_confirmed_with_provider_limits(value: usize) -> Self {
        Self(value)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Tile request channel from the main thread was broken.")]
    RequestChannelBroken,

    #[error("Tile channel to the main thread was broken.")]
    TileChannelClosed,

    #[error("Tile channel to the main thread was full.")]
    TileChannelFull,

    #[error("Fetch error: {0}")]
    Fetch(String),

    #[error(transparent)]
    Tile(#[from] TileError),

    #[error("Poison error.")]
    Poisoned,
}

impl From<futures::channel::mpsc::SendError> for Error {
    fn from(error: futures::channel::mpsc::SendError) -> Self {
        if error.is_disconnected() {
            Error::TileChannelClosed
        } else {
            Error::TileChannelFull
        }
    }
}

impl<T> From<std::sync::PoisonError<T>> for Error {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        Error::Poisoned
    }
}

/// Download and decode the tile.
async fn fetch_and_decode(
    fetch: &impl Fetch,
    tile_id: TileId,
    egui_ctx: &Context,
) -> Result<(TileId, Tile), Error> {
    fetch_and_decode_impl(fetch, tile_id, egui_ctx)
        .await
        .map(|tile| (tile_id, tile))
}

async fn fetch_and_decode_impl(
    fetch: &impl Fetch,
    tile_id: TileId,
    egui_ctx: &Context,
) -> Result<Tile, Error> {
    let image = fetch
        .fetch(tile_id)
        .await
        .map_err(|e| Error::Fetch(e.to_string()))?;
    Ok(Tile::new(&image, egui_ctx)?)
}

async fn fetch_complete(
    mut tile_tx: Sender<(TileId, Tile)>,
    egui_ctx: Context,
    result: Result<(TileId, Tile), Error>,
) -> Result<(), Error> {
    match result {
        Ok((tile_id, tile)) => {
            tile_tx.send((tile_id, tile)).await.map_err(Error::from)?;
            egui_ctx.request_repaint();
        }
        Err(e) => {
            // It would probably be more consistent to push it to the caller, but it's not that
            // important right now.
            log::warn!("{e}");
        }
    };

    Ok(())
}

async fn fetch_continuously_impl(
    fetch: impl Fetch,
    stats: Arc<Mutex<Stats>>,
    mut request_rx: Receiver<TileId>,
    tile_tx: Sender<(TileId, Tile)>,
    egui_ctx: Context,
) -> Result<(), Error> {
    let mut outstanding = Vec::new();

    loop {
        if outstanding.is_empty() {
            // Only new downloads might be requested.
            let tile_id = request_rx.next().await.ok_or(Error::RequestChannelBroken)?;
            let f = fetch_and_decode(&fetch, tile_id, &egui_ctx);
            outstanding.push(Box::pin(f));
        } else if outstanding.len() < fetch.max_concurrency() {
            // New downloads might be requested or ongoing downloads might be completed.
            match select(request_rx.next(), select_all(outstanding.drain(..))).await {
                // New download was requested.
                Either::Left((request, remaining)) => {
                    let tile_id = request.ok_or(Error::RequestChannelBroken)?;
                    let f = fetch_and_decode(&fetch, tile_id, &egui_ctx);
                    outstanding = remaining.into_inner();
                    outstanding.push(Box::pin(f));
                }
                // Ongoing download was completed.
                Either::Right(((result, _, remaining), _)) => {
                    fetch_complete(tile_tx.to_owned(), egui_ctx.to_owned(), result).await?;
                    outstanding = remaining;
                }
            }
        } else {
            // Only ongoing downloads might be completed.
            let (result, _, remaining) = select_all(outstanding.drain(..)).await;
            fetch_complete(tile_tx.to_owned(), egui_ctx.to_owned(), result).await?;
            outstanding = remaining;
        }

        // Update stats.
        let mut stats = stats.lock()?;
        stats.in_progress = outstanding.len();
    }
}

/// Continuously fetch tiles requested via request channel.
pub(crate) async fn fetch_continuously(
    fetch: impl Fetch,
    stats: Arc<Mutex<Stats>>,
    request_rx: futures::channel::mpsc::Receiver<TileId>,
    tile_tx: futures::channel::mpsc::Sender<(TileId, Tile)>,
    egui_ctx: Context,
) {
    match fetch_continuously_impl(fetch, stats, request_rx, tile_tx, egui_ctx).await {
        Ok(()) | Err(Error::TileChannelClosed) | Err(Error::RequestChannelBroken) => {
            log::debug!("Tile fetch loop finished.");
        }
        Err(error) => {
            log::error!("Tile fetch loop failed: {error}.");
        }
    }
}

pub trait Fetch {
    type Error: std::error::Error + Sync + Send;

    #[cfg(target_arch = "wasm32")]
    fn fetch(&self, tile_id: TileId) -> impl Future<Output = Result<Bytes, Self::Error>>;

    #[cfg(not(target_arch = "wasm32"))]
    fn fetch(&self, tile_id: TileId) -> impl Future<Output = Result<Bytes, Self::Error>> + Send;

    fn max_concurrency(&self) -> usize;
}
