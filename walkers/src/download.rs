use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use bytes::Bytes;
use egui::Context;
use futures::{
    SinkExt, StreamExt,
    future::{Either, select, select_all},
};
use reqwest_middleware::ClientWithMiddleware;

use crate::{TileId, http_tiles::HttpStats, io::http_client, sources::TileSource, tiles::{Texture, TileError}};

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
    #[error(transparent)]
    HttpMiddleware(#[from] reqwest_middleware::Error),

    #[error(transparent)]
    Http(#[from] reqwest::Error),

    #[error(transparent)]
    Tile(#[from] TileError),

    #[error("Tile request channel from the main thread was broken.")]
    RequestChannelBroken,

    #[error("Tile channel to the main thread was broken.")]
    TileChannelClosed,

    #[error("Tile channel to the main thread was full.")]
    TileChannelFull,

    #[error("Poison error.")]
    Poisoned,

    #[error("Fetch error: {0}")]
    Fetch(String),
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
async fn download_and_decode(
    fetch: &impl Fetch,
    tile_id: TileId,
    egui_ctx: &Context,
) -> Result<(TileId, Texture), Error> {
    download_and_decode_impl(fetch, tile_id, egui_ctx)
        .await
        .map(|tile| (tile_id, tile))
}

async fn download_and_decode_impl(
    fetch: &impl Fetch,
    tile_id: TileId,
    egui_ctx: &Context,
) -> Result<Texture, Error> {
    let image = fetch
        .fetch(tile_id)
        .await
        .map_err(|e| Error::Fetch(e.to_string()))?;
    Ok(Texture::new(&image, egui_ctx)?)
}

async fn download_complete(
    mut tile_tx: futures::channel::mpsc::Sender<(TileId, Texture)>,
    egui_ctx: Context,
    result: Result<(TileId, Texture), Error>,
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

async fn download_continuously_impl(
    fetch: impl Fetch,
    stats: Arc<Mutex<HttpStats>>,
    mut request_rx: futures::channel::mpsc::Receiver<TileId>,
    tile_tx: futures::channel::mpsc::Sender<(TileId, Texture)>,
    egui_ctx: Context,
) -> Result<(), Error> {
    let mut downloads = Vec::new();

    loop {
        if downloads.is_empty() {
            // Only new downloads might be requested.
            let tile_id = request_rx.next().await.ok_or(Error::RequestChannelBroken)?;
            let download = download_and_decode(&fetch, tile_id, &egui_ctx);
            downloads.push(Box::pin(download));
        } else if downloads.len() < fetch.max_concurrency() {
            // New downloads might be requested or ongoing downloads might be completed.
            match select(request_rx.next(), select_all(downloads.drain(..))).await {
                // New download was requested.
                Either::Left((request, remaining_downloads)) => {
                    let tile_id = request.ok_or(Error::RequestChannelBroken)?;
                    let download = download_and_decode(&fetch, tile_id, &egui_ctx);
                    downloads = remaining_downloads.into_inner();
                    downloads.push(Box::pin(download));
                }
                // Ongoing download was completed.
                Either::Right(((result, _, remaining_downloads), _)) => {
                    download_complete(tile_tx.to_owned(), egui_ctx.to_owned(), result).await?;
                    downloads = remaining_downloads;
                }
            }
        } else {
            // Only ongoing downloads might be completed.
            let (result, _, remaining_downloads) = select_all(downloads.drain(..)).await;
            download_complete(tile_tx.to_owned(), egui_ctx.to_owned(), result).await?;
            downloads = remaining_downloads;
        }

        // Update stats.
        let mut stats = stats.lock()?;
        stats.in_progress = downloads.len();
    }
}

/// Continuously download tiles requested via request channel.
pub(crate) async fn download_continuously(
    fetch: impl Fetch,
    stats: Arc<Mutex<HttpStats>>,
    request_rx: futures::channel::mpsc::Receiver<TileId>,
    tile_tx: futures::channel::mpsc::Sender<(TileId, Texture)>,
    egui_ctx: Context,
) {
    match download_continuously_impl(fetch, stats, request_rx, tile_tx, egui_ctx).await {
        Ok(()) | Err(Error::TileChannelClosed) | Err(Error::RequestChannelBroken) => {
            log::debug!("Tile download loop finished.");
        }
        Err(error) => {
            log::error!("Tile download loop failed: {error}.");
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HttpFetchError {
    #[error(transparent)]
    HttpMiddleware(#[from] reqwest_middleware::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
}

pub trait Fetch {
    type Error: std::error::Error + Sync + Send + 'static;

    fn fetch(
        &self,
        tile_id: TileId,
    ) -> impl std::future::Future<Output = Result<Bytes, Self::Error>> + std::marker::Send;

    fn max_concurrency(&self) -> usize;
}

pub struct HttpFetch<S>
where
    S: TileSource + Send + 'static,
{
    pub source: S,
    pub http_options: HttpOptions,
    pub client: ClientWithMiddleware,
}

impl<S> HttpFetch<S>
where
    S: TileSource + Sync + Send,
{
    pub fn new(source: S, http_options: HttpOptions) -> Result<Self, Error> {
        let client = http_client(&http_options)?;
        Ok(Self {
            source,
            http_options,
            client,
        })
    }
}

impl<S> Fetch for HttpFetch<S>
where
    S: TileSource + Sync + Send,
{
    type Error = HttpFetchError;

    async fn fetch(&self, tile_id: TileId) -> Result<Bytes, Self::Error> {
        let url = self.source.tile_url(tile_id);
        log::trace!("Downloading '{url}'.");
        let image = self.client.get(&url).send().await?;
        log::trace!("Downloaded '{}': {:?}.", url, image.status());
        Ok(image.error_for_status()?.bytes().await?)
    }

    fn max_concurrency(&self) -> usize {
        self.http_options.max_parallel_downloads.0
    }
}
