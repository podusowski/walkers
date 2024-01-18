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

/// Controls how [`crate::Tiles`] use the HTTP protocol, such as caching.
#[derive(Default)]
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
    egui_ctx: &Context,
) -> Download {
    log::debug!("Downloading '{}'.", url);
    Download {
        tile_id,
        result: download_and_decode_impl(client, url, egui_ctx).await,
    }
}

async fn download_and_decode_impl(
    client: &ClientWithMiddleware,
    url: String,
    egui_ctx: &Context,
) -> Result<Texture, Error> {
    let image = client
        .get(&url)
        .header(USER_AGENT, "Walkers")
        .send()
        .await
        .map_err(Error::HttpMiddleware)?;

    log::debug!("Downloaded '{}': {:?}.", url, image.status());

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

impl<F> Downloads<F> {
    fn new(downloads: Vec<Pin<Box<F>>>) -> Self {
        if downloads.is_empty() {
            Self::None
        } else if downloads.len() < 6 {
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
    // Keep outside the loop to reuse it as much as possible.
    let client = http_client(http_options);
    let mut downloads = Downloads::None;

    loop {
        let request = request_rx.next();

        downloads = match downloads {
            Downloads::None => {
                let request = request.await.ok_or(())?;
                let url = source.tile_url(request);
                let download = download_and_decode(&client, request, url, &egui_ctx);
                Downloads::Ongoing(vec![Box::pin(download)])
            }
            Downloads::Ongoing(ref mut dls) => {
                let download = select_all(dls.drain(..));
                match select(request, download).await {
                    Either::Left((request, r)) => {
                        let request = request.ok_or(())?;
                        let url = source.tile_url(request);
                        let download = download_and_decode(&client, request, url, &egui_ctx);
                        let mut downloads = r.into_inner();
                        downloads.push(Box::pin(download));
                        Downloads::new(downloads)
                    }
                    Either::Right((downloads, _)) => {
                        let (result, _, rest) = downloads;
                        download_complete(
                            tile_tx.to_owned(),
                            egui_ctx.to_owned(),
                            result.tile_id,
                            result.result,
                        )
                        .await?;
                        Downloads::new(rest)
                    }
                }
            }
            Downloads::OngoingSaturated(ref mut downloads) => {
                let download = select_all(downloads.drain(..));
                let (result, _, rest) = download.await;
                download_complete(
                    tile_tx.to_owned(),
                    egui_ctx.to_owned(),
                    result.tile_id,
                    result.result,
                )
                .await?;
                Downloads::Ongoing(rest)
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
