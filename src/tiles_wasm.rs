use std::collections::hash_map::Entry;
use std::sync::mpsc::TryRecvError;
use std::{collections::HashMap, sync::Arc};

use egui::{pos2, Color32, Context, Mesh, Rect, Vec2};
use egui_extras::RetainedImage;
use reqwest::header::USER_AGENT;

use crate::mercator::TileId;

#[derive(Clone)]
pub struct Tile {
    image: Arc<RetainedImage>,
}

impl Tile {
    fn from_image_bytes(image: &[u8]) -> Result<Self, String> {
        RetainedImage::from_image_bytes("debug_name", image).map(|image| Self {
            image: Arc::new(image),
        })
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
    source: Box<dyn Fn(TileId) -> String + Send + 'static>,

    cache: HashMap<TileId, Option<Tile>>,

    /// Tiles that got downloaded and should be put in the cache.
    tile_rx: std::sync::mpsc::Receiver<(TileId, Tile)>,

    tile_tx: std::sync::mpsc::Sender<(TileId, Tile)>,
}

impl Tiles {
    pub fn new<S>(source: S, egui_ctx: Context) -> Self
    where
        S: Fn(TileId) -> String + Send + 'static,
    {
        let (tx, rx) = std::sync::mpsc::channel();

        Self {
            source: Box::new(source),
            cache: Default::default(),
            tile_rx: rx,
            tile_tx: tx,
        }
    }

    /// Return a tile if already in cache, schedule a download otherwise.
    pub fn at(&mut self, tile_id: TileId) -> Option<Tile> {
        match self.tile_rx.try_recv() {
            Ok((tile_id, tile)) => {
                self.cache.insert(tile_id, Some(tile));
            }
            Err(TryRecvError::Empty) => {
                // Just ignore. It means that no new tile was downloaded.
            }
            Err(TryRecvError::Disconnected) => panic!("IO thread is dead"),
        }

        match self.cache.entry(tile_id) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                entry.insert(None);

                let url = (self.source)(tile_id);
                log::info!("Getting {}", url);

                let request = ehttp::Request::get(url);
                let tile_tx = self.tile_tx.clone();
                ehttp::fetch(request, move |result: ehttp::Result<ehttp::Response>| {
                    if let Ok(response) = result {
                        log::info!("{:?}", response);
                        if response.ok {
                            assert_eq!(200, response.status);
                            let tile = Tile::from_image_bytes(&response.bytes);
                            tile_tx.send((tile_id, tile.unwrap())).unwrap();
                        }
                    }
                });

                None
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error(transparent)]
    Http(reqwest::Error),

    #[error("error while decoding the image: {0}")]
    Image(String),
}
