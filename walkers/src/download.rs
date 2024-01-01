use std::path::PathBuf;

use egui::Context;
use futures::{SinkExt, StreamExt};
use image::ImageError;
use reqwest::header::USER_AGENT;
use reqwest_middleware::ClientWithMiddleware;

use crate::{io::http_client, mercator::TileId, providers::TileSource, tiles::Texture};

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

/// Download and decode the tile.
async fn download_and_decode(
    client: &ClientWithMiddleware,
    url: &str,
    egui_ctx: &Context,
) -> Result<Texture, Error> {
    let image = client
        .get(url)
        .header(USER_AGENT, "Walkers")
        .send()
        .await
        .map_err(Error::HttpMiddleware)?;

    log::debug!("Downloaded {:?}.", image.status());

    let image = image
        .error_for_status()
        .map_err(Error::Http)?
        .bytes()
        .await
        .map_err(Error::Http)?;

    Texture::new(&image, egui_ctx).map_err(Error::Image)
}

async fn download_continuously_impl<S>(
    source: S,
    http_options: HttpOptions,
    mut request_rx: futures::channel::mpsc::Receiver<TileId>,
    mut tile_tx: futures::channel::mpsc::Sender<(TileId, Texture)>,
    egui_ctx: Context,
) -> Result<(), ()>
where
    S: TileSource + Send + 'static,
{
    // Keep outside the loop to reuse it as much as possible.
    let client = http_client(http_options);

    loop {
        let request = request_rx.next().await.ok_or(())?;
        let url = source.tile_url(request);

        log::debug!("Getting {:?} from {}.", request, url);

        match download_and_decode(&client, &url, &egui_ctx).await {
            Ok(tile) => {
                tile_tx.send((request, tile)).await.map_err(|_| ())?;
                egui_ctx.request_repaint();
            }
            Err(e) => {
                log::warn!("Could not download '{}': {}", &url, e);
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
