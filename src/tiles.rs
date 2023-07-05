use std::collections::hash_map::Entry;
use std::{collections::HashMap, sync::Arc};

use egui::{pos2, Color32, Context, Mesh, Rect, Vec2};
use egui_extras::RetainedImage;
use reqwest::header::USER_AGENT;

use crate::mercator::TileId;
use crate::tokio::TokioRuntimeThread;

#[derive(Clone)]
pub struct Tile {
    image: Arc<RetainedImage>,
}

impl Tile {
    fn new(image: &[u8]) -> Self {
        Self {
            image: Arc::new(RetainedImage::from_image_bytes("debug_name", image).unwrap()),
        }
    }

    pub fn rect(&self, screen_position: Vec2) -> Rect {
        let tile_size = pos2(self.image.width() as f32, self.image.height() as f32);
        Rect::from_two_pos(
            screen_position.to_pos2(),
            (screen_position + tile_size.to_vec2()).to_pos2(),
        )
    }

    pub fn mesh(&self, screen_position: Vec2, ctx: &Context) -> Mesh {
        let mut mesh = Mesh::with_texture(self.image.texture_id(ctx));
        mesh.add_rect_with_uv(
            self.rect(screen_position),
            Rect::from_min_max(pos2(0., 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        mesh
    }
}

/// Downloads and keeps cache of the tiles. It must persist between frames.
pub struct Tiles {
    cache: HashMap<TileId, Option<Tile>>,

    /// Tiles to be downloaded by the IO thread.
    request_tx: tokio::sync::mpsc::Sender<TileId>,

    /// Tiles that got downloaded and should be put in the cache.
    tile_rx: tokio::sync::mpsc::Receiver<(TileId, Tile)>,

    #[allow(dead_code)] // Significant Drop
    tokio_runtime_thread: TokioRuntimeThread,
}

pub fn openstreetmap(tile_id: TileId) -> String {
    format!(
        "https://tile.openstreetmap.org/{}/{}/{}.png",
        tile_id.zoom, tile_id.x, tile_id.y
    )
}

impl Tiles {
    pub fn new<S>(source: S, egui_ctx: Context) -> Self
    where
        S: Fn(TileId) -> String + Send + 'static,
    {
        let tokio_runtime_thread = TokioRuntimeThread::new();

        // Minimum value which didn't cause any stalls while testing.
        let channel_size = 20;

        let (request_tx, request_rx) = tokio::sync::mpsc::channel(channel_size);
        let (tile_tx, tile_rx) = tokio::sync::mpsc::channel(channel_size);
        tokio_runtime_thread
            .runtime
            .spawn(download(source, request_rx, tile_tx, egui_ctx));
        Self {
            cache: Default::default(),
            request_tx,
            tile_rx,
            tokio_runtime_thread,
        }
    }

    pub fn at(&mut self, tile_id: TileId) -> Option<Tile> {
        // Just take one at the time.
        if let Ok((tile_id, tile)) = self.tile_rx.try_recv() {
            self.cache.insert(tile_id, Some(tile));
        }

        match self.cache.entry(tile_id) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                if let Ok(()) = self.request_tx.try_send(tile_id) {
                    log::debug!("Requested tile: {:?}", tile_id);
                    entry.insert(None);
                } else {
                    log::debug!("Request queue is full.");
                }
                None
            }
        }
    }
}

async fn download<S>(
    source: S,
    mut request_rx: tokio::sync::mpsc::Receiver<TileId>,
    tile_tx: tokio::sync::mpsc::Sender<(TileId, Tile)>,
    egui_ctx: Context,
) where
    S: Fn(TileId) -> String + Send + 'static,
{
    let client = reqwest::Client::new();
    loop {
        if let Some(requested) = request_rx.recv().await {
            log::debug!("Starting the download of {:?}.", requested);

            let url = source(requested);

            let image = client
                .get(url)
                .header(USER_AGENT, "Walkers")
                .send()
                .await
                .unwrap();

            log::debug!("Downloaded {:?}.", image.status());

            if image.status().is_success() {
                let image = image.bytes().await.unwrap();
                if tile_tx.send((requested, Tile::new(&image))).await.is_err() {
                    // GUI thread died.
                    break;
                }
                egui_ctx.request_repaint();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use egui::Context;

    use crate::{mercator::TileId, tiles::openstreetmap, Tiles};

    #[test]
    fn download_single_tile() {
        let _ = env_logger::try_init();

        let mut server = mockito::Server::new();
        let valid_png = include_bytes!("valid.png");
        let tile_mock = server
            .mock("GET", "/3/1/2.png")
            .with_body(valid_png)
            .create();

        let tile_id = TileId {
            x: 1,
            y: 2,
            zoom: 3,
        };

        let url = server.url();

        let source = move |tile_id: TileId| {
            format!("{}/{}/{}/{}.png", url, tile_id.zoom, tile_id.x, tile_id.y)
        };

        let mut tiles = Tiles::new(source, Context::default());

        // First query start the download, but it will always return None.
        assert!(tiles.at(tile_id).is_none());

        // Eventually it gets downloaded and become available in cache.
        while tiles.at(tile_id).is_none() {}

        tile_mock.assert();
    }
}
