use std::path::PathBuf;

use egui::Context;
use futures::{
    future::{select, select_all, Either},
    SinkExt, StreamExt,
};
use image::ImageError;
use reqwest::header::USER_AGENT;
use reqwest_middleware::ClientWithMiddleware;

use crate::{io::http_client, mercator::TileId, sources::TileSource, tiles::Texture};

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
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error(transparent)]
    HttpMiddleware(reqwest_middleware::Error),

    #[error(transparent)]
    Http(reqwest::Error),

    #[error(transparent)]
    Image(ImageError),

    #[error("Tile request channel from the main thread was broken.")]
    RequestChannelBroken,

    #[error("Tile channel to the main thread was broken.")]
    TileChannelClosed,

    #[error("Tile channel to the main thread was full.")]
    TileChannelFull,
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

struct Download {
    tile_id: TileId,
    result: Result<Texture, Error>,
}

/// Download and decode the tile.
async fn download_and_decode(
    client: &ClientWithMiddleware,
    tile_id: TileId,
    url: String,
    user_agent: Option<&HeaderValue>,
    egui_ctx: &Context,
) -> Download {
    log::trace!("Downloading '{}'.", url);
    Download {
        tile_id,
        result: download_and_decode_impl(client, url, user_agent, egui_ctx).await,
    }
}

async fn download_and_decode_impl(
    client: &ClientWithMiddleware,
    url: String,
    user_agent: Option<&HeaderValue>,
    egui_ctx: &Context,
) -> Result<Texture, Error> {
    let mut image_request = client.get(&url);

    if let Some(user_agent) = user_agent {
        image_request = image_request.header(USER_AGENT, user_agent);
    }

    let image = image_request.send().await.map_err(Error::HttpMiddleware)?;

    log::trace!("Downloaded '{}': {:?}.", url, image.status());

    let image = image
        .error_for_status()
        .map_err(Error::Http)?
        .bytes()
        .await
        .map_err(Error::Http)?;

    Texture::new(&image, egui_ctx).map_err(Error::Image)
}

async fn download_complete(
    mut tile_tx: futures::channel::mpsc::Sender<(TileId, Texture)>,
    egui_ctx: Context,
    download: Download,
) -> Result<(), Error> {
    match download.result {
        Ok(tile) => {
            tile_tx
                .send((download.tile_id, tile))
                .await
                .map_err(Error::from)?;
            egui_ctx.request_repaint();
        }
        Err(e) => {
            // It would probably be more consistent to push it to the caller, but it's not that
            // important right now.
            log::warn!("{}", e);
        }
    };

    Ok(())
}

/// Maximum number of parallel downloads. Following modern browsers' behavior.
/// https://stackoverflow.com/questions/985431/max-parallel-http-connections-in-a-browser
pub(crate) const MAX_PARALLEL_DOWNLOADS: usize = 6;

async fn download_continuously_impl<S>(
    source: S,
    http_options: HttpOptions,
    mut request_rx: futures::channel::mpsc::Receiver<TileId>,
    tile_tx: futures::channel::mpsc::Sender<(TileId, Texture)>,
    egui_ctx: Context,
) -> Result<(), Error>
where
    S: TileSource + Send + 'static,
{
    let user_agent = http_options.user_agent.clone();

    // Keep outside the loop to reuse it as much as possible.
    let client = http_client(http_options);
    let mut downloads = Vec::new();

    loop {
        if downloads.is_empty() {
            // Only new downloads might be requested.
            let tile_id = request_rx.next().await.ok_or(Error::RequestChannelBroken)?;
            let url = source.tile_url(tile_id);
            let download =
                download_and_decode(&client, tile_id, url, user_agent.as_ref(), &egui_ctx);
            downloads.push(Box::pin(download));
        } else if downloads.len() < MAX_PARALLEL_DOWNLOADS {
            // New downloads might be requested or ongoing downloads might be completed.
            let download = select_all(downloads.drain(..));
            match select(request_rx.next(), download).await {
                // New download was requested.
                Either::Left((request, remaining_downloads)) => {
                    let tile_id = request.ok_or(Error::RequestChannelBroken)?;
                    let url = source.tile_url(tile_id);
                    let download =
                        download_and_decode(&client, tile_id, url, user_agent.as_ref(), &egui_ctx);
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
    }
}

/// Continuously download tiles requested via request channel.
pub(crate) async fn download_continuously<S>(
    source: S,
    http_options: HttpOptions,
    request_rx: futures::channel::mpsc::Receiver<TileId>,
    tile_tx: futures::channel::mpsc::Sender<(TileId, Texture)>,
    egui_ctx: Context,
) where
    S: TileSource + Send + 'static,
{
    match download_continuously_impl(source, http_options, request_rx, tile_tx, egui_ctx).await {
        Ok(()) | Err(Error::TileChannelClosed) | Err(Error::RequestChannelBroken) => {
            log::debug!("Tile download loop finished.");
        }
        Err(error) => {
            log::error!("Tile download loop failed: {}.", error);
        }
    }
}
