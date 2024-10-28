use std::{path::PathBuf, pin::Pin};

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
    pub user_agent: HeaderValue,
}

impl Default for HttpOptions {
    fn default() -> Self {
        Self {
            cache: None,
            user_agent: HeaderValue::from_static(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION"),
            )),
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
    user_agent: &HeaderValue,
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
    user_agent: &HeaderValue,
    egui_ctx: &Context,
) -> Result<Texture, Error> {
    let image = client
        .get(&url)
        .header(USER_AGENT, user_agent)
        .send()
        .await
        .map_err(Error::HttpMiddleware)?;

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
    tile_id: TileId,
    result: Result<Texture, Error>,
) -> Result<(), ()> {
    match result {
        Ok(tile) => {
            tile_tx.send((tile_id, tile)).await.map_err(|_| ())?;
            egui_ctx.request_repaint();
        }
        Err(e) => {
            log::warn!("{}", e);
        }
    };

    Ok(())
}

enum Downloads<F> {
    None,
    Ongoing(Vec<Pin<Box<F>>>),
    OngoingSaturated(Vec<Pin<Box<F>>>),
}

/// Maximum number of parallel downloads. Following modern browsers' behavior.
/// https://stackoverflow.com/questions/985431/max-parallel-http-connections-in-a-browser
const MAX_PARALLEL_DOWNLOADS: usize = 6;

impl<F> Downloads<F> {
    fn new(downloads: Vec<Pin<Box<F>>>) -> Self {
        if downloads.is_empty() {
            Self::None
        } else if downloads.len() < MAX_PARALLEL_DOWNLOADS {
            Self::Ongoing(downloads)
        } else {
            Self::OngoingSaturated(downloads)
        }
    }
}

async fn download_continuously_impl<S>(
    source: S,
    http_options: HttpOptions,
    mut request_rx: futures::channel::mpsc::Receiver<TileId>,
    tile_tx: futures::channel::mpsc::Sender<(TileId, Texture)>,
    egui_ctx: Context,
) -> Result<(), ()>
where
    S: TileSource + Send + 'static,
{
    let user_agent = http_options.user_agent.to_owned();

    // Keep outside the loop to reuse it as much as possible.
    let client = http_client(http_options);
    let mut downloads = Downloads::None;

    loop {
        downloads = match downloads {
            Downloads::None => {
                let request = request_rx.next().await.ok_or(())?;
                let url = source.tile_url(request);
                let download = download_and_decode(&client, request, url, &user_agent, &egui_ctx);
                Downloads::Ongoing(vec![Box::pin(download)])
            }
            Downloads::Ongoing(ref mut downloads) => {
                let download = select_all(downloads.drain(..));
                match select(request_rx.next(), download).await {
                    // New download was requested.
                    Either::Left((request, downloads)) => {
                        let request = request.ok_or(())?;
                        let url = source.tile_url(request);
                        let download =
                            download_and_decode(&client, request, url, &user_agent, &egui_ctx);
                        let mut downloads = downloads.into_inner();
                        downloads.push(Box::pin(download));
                        Downloads::new(downloads)
                    }
                    // Ongoing download was completed.
                    Either::Right(((result, _, downloads), _)) => {
                        download_complete(
                            tile_tx.to_owned(),
                            egui_ctx.to_owned(),
                            result.tile_id,
                            result.result,
                        )
                        .await?;
                        Downloads::new(downloads)
                    }
                }
            }
            Downloads::OngoingSaturated(ref mut downloads) => {
                let (result, _, downloads) = select_all(downloads.drain(..)).await;
                download_complete(
                    tile_tx.to_owned(),
                    egui_ctx.to_owned(),
                    result.tile_id,
                    result.result,
                )
                .await?;
                Downloads::Ongoing(downloads)
            }
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
    if download_continuously_impl(source, http_options, request_rx, tile_tx, egui_ctx)
        .await
        .is_err()
    {
        log::error!("Error from IO runtime.");
    }
}
