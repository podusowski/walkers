use crate::{
    Stats, TileId,
    tiles::{Tile, TileError},
};
use bytes::Bytes;
use egui::Context;
use futures::{
    SinkExt, StreamExt,
    channel::mpsc::{Receiver, Sender},
    future::{Either, select, select_all},
};
/// Asynchronous fetching loop.
use std::sync::{Arc, Mutex};

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

pub trait TileFactory {
    fn create_tile(&self, data: &Bytes, zoom: u8) -> Result<Tile, TileError>;
}

/// Download and decode the tile.
async fn fetch_and_decode(
    fetch: &impl Fetch,
    tile_id: TileId,
    tile_factory: &impl TileFactory,
) -> Result<(TileId, Tile), Error> {
    let data = fetch
        .fetch(tile_id)
        .await
        .map_err(|e| Error::Fetch(e.to_string()))?;
    Ok(tile_factory
        .create_tile(&data, tile_id.zoom)
        .map(|tile| (tile_id, tile))?)
}

/// Deliver the fetched tile to the main thread.
async fn fetch_complete(
    mut tile_tx: Sender<(TileId, Tile)>,
    egui_ctx: Context,
    result: Result<(TileId, Tile), Error>,
) -> Result<(), Error> {
    match result {
        Ok((tile_id, tile)) => {
            tile_tx.send((tile_id, tile)).await?;
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
    tile_factory: impl TileFactory,
    egui_ctx: Context,
) -> Result<(), Error> {
    let mut outstanding = Vec::new();

    loop {
        if outstanding.is_empty() {
            // Only new fetches might be requested.
            let tile_id = request_rx.next().await.ok_or(Error::RequestChannelBroken)?;
            let f = fetch_and_decode(&fetch, tile_id, &tile_factory);
            outstanding.push(Box::pin(f));
        } else if outstanding.len() < fetch.max_concurrency() {
            // New fetches might be requested or ongoing fetches might be completed.
            match select(request_rx.next(), select_all(outstanding.drain(..))).await {
                // New fetch was requested.
                Either::Left((request, remaining)) => {
                    let tile_id = request.ok_or(Error::RequestChannelBroken)?;
                    let f = fetch_and_decode(&fetch, tile_id, &tile_factory);
                    outstanding = remaining.into_inner();
                    outstanding.push(Box::pin(f));
                }
                // Ongoing fetch was completed.
                Either::Right(((result, _, remaining), _)) => {
                    fetch_complete(tile_tx.to_owned(), egui_ctx.to_owned(), result).await?;
                    outstanding = remaining;
                }
            }
        } else {
            // Only ongoing fetches might be completed.
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
    request_rx: Receiver<TileId>,
    tile_tx: Sender<(TileId, Tile)>,
    egui_ctx: Context,
    tile_factory: impl TileFactory,
) {
    match fetch_continuously_impl(fetch, stats, request_rx, tile_tx, tile_factory, egui_ctx).await {
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
