use std::hash::Hash;

use egui::Context;
use futures::{SinkExt, StreamExt};
use reqwest::header::USER_AGENT;

use crate::cache::TileCache;
use crate::mercator::TileId;
use crate::providers::TileSource;
use crate::tiles::Texture;

/// Download and decode the tile.
async fn download_tile(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, reqwest::Error> {
    let image = client.get(url).header(USER_AGENT, "Walkers").send().await?;

    log::debug!("Downloaded {:?}.", image.status());

    let image = image.error_for_status()?.bytes().await?;

    Ok(image.to_vec())
}

async fn download_continuously_impl<C, S>(
    source: S,
    mut cache: C,
    mut request_rx: futures::channel::mpsc::Receiver<TileId>,
    mut tile_tx: futures::channel::mpsc::Sender<(TileId, Texture)>,
    egui_ctx: Context,
) -> Result<(), ()>
where
    C: TileCache + Send + 'static,
    C::Error: std::fmt::Display,
    S: TileSource + Hash + Send + 'static,
{
    // Keep outside the loop to reuse it as much as possible.
    let client = reqwest::Client::new();

    loop {
        let request = request_rx.next().await.ok_or(())?;

        let cached_bytes = match cache.read(&source, request) {
            Ok(cached) => cached,
            Err(e) => {
                log::warn!("Failed to load tile from cache: {}.", e);
                None
            }
        };

        let bytes = if let Some(bytes) = cached_bytes {
            Some(bytes)
        } else {
            let url = source.tile_url(request);
            log::debug!("Getting {:?} from {}.", request, url);
            match download_tile(&client, &url).await {
                Ok(bytes) => {
                    if let Err(e) = cache.write(&source, request, &bytes) {
                        log::warn!("Failed to write tile to cache: {}", e);
                    }
                    Some(bytes)
                }
                Err(e) => {
                    log::warn!("Could not download '{}': {}", &url, e);
                    None
                }
            }
        };

        if let Some(bytes) = bytes {
            match Texture::new(&bytes, &egui_ctx) {
                Ok(tile) => {
                    tile_tx.send((request, tile)).await.map_err(|_| ())?;
                    egui_ctx.request_repaint();
                }
                Err(e) => {
                    log::warn!("Failed to decode image: {}", e);
                }
            }
        }
    }
}

/// Continuously download tiles requested via request channel.
pub(crate) async fn download_continuously<C, S>(
    source: S,
    cache: C,
    request_rx: futures::channel::mpsc::Receiver<TileId>,
    tile_tx: futures::channel::mpsc::Sender<(TileId, Texture)>,
    egui_ctx: Context,
) where
    C: TileCache + Send + 'static,
    C::Error: std::fmt::Display,
    S: TileSource + Hash + Send + 'static,
{
    if download_continuously_impl(source, cache, request_rx, tile_tx, egui_ctx)
        .await
        .is_err()
    {
        log::error!("Error from IO runtime.");
    }
}
