use std::collections::hash_map::Entry;
use std::collections::HashMap;

use egui::{pos2, Color32, Context, Mesh, Rect, Vec2};
use egui::{ColorImage, TextureHandle};
use image::ImageError;

use crate::download::download_continuously;
use crate::io::Runtime;
use crate::mercator::TileId;
use crate::providers::{Attribution, TileSource};

pub(crate) fn rect(screen_position: Vec2, tile_size: u32) -> Rect {
    Rect::from_min_size(screen_position.to_pos2(), Vec2::splat(tile_size as f32))
}

#[derive(Clone)]
pub struct Texture(TextureHandle);

impl Texture {
    pub fn new(image: &[u8], ctx: &Context) -> Result<Self, ImageError> {
        let image = image::load_from_memory(image)?.to_rgba8();
        let pixels = image.as_flat_samples();
        let image = ColorImage::from_rgba_unmultiplied(
            [image.width() as _, image.height() as _],
            pixels.as_slice(),
        );

        Ok(Self::from_color_image(image, ctx))
    }

    /// Load the texture from egui's [`ColorImage`].
    pub fn from_color_image(color_image: ColorImage, ctx: &Context) -> Self {
        Self(ctx.load_texture("image", color_image, Default::default()))
    }

    pub(crate) fn size(&self) -> Vec2 {
        self.0.size_vec2()
    }

    pub(crate) fn mesh(&self, screen_position: Vec2, tile_size: u32) -> Mesh {
        self.mesh_with_rect(rect(screen_position, tile_size))
    }

    pub(crate) fn mesh_with_rect(&self, rect: Rect) -> Mesh {
        let mut mesh = Mesh::with_texture(self.0.id());
        mesh.add_rect_with_uv(
            rect,
            Rect::from_min_max(pos2(0., 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        mesh
    }
}

pub trait TilesManager {
    fn at(&mut self, tile_id: TileId) -> Option<Texture>;
    fn attribution(&self) -> Attribution;
    fn tile_size(&self) -> u32;
}

/// Downloads and keeps cache of the tiles. It must persist between frames.
pub struct Tiles {
    attribution: Attribution,

    cache: HashMap<TileId, Option<Texture>>,

    /// Tiles to be downloaded by the IO thread.
    request_tx: futures::channel::mpsc::Sender<TileId>,

    /// Tiles that got downloaded and should be put in the cache.
    tile_rx: futures::channel::mpsc::Receiver<(TileId, Texture)>,

    #[allow(dead_code)] // Significant Drop
    runtime: Runtime,

    pub tile_size: u32,
}

impl Tiles {
    pub fn new<S>(source: S, egui_ctx: Context) -> Self
    where
        S: TileSource + Send + 'static,
    {
        // Minimum value which didn't cause any stalls while testing.
        let channel_size = 20;

        let (request_tx, request_rx) = futures::channel::mpsc::channel(channel_size);
        let (tile_tx, tile_rx) = futures::channel::mpsc::channel(channel_size);
        let attribution = source.attribution();
        let tile_size = source.tile_size();
        let runtime = Runtime::new(download_continuously(source, request_rx, tile_tx, egui_ctx));

        Self {
            attribution,
            cache: Default::default(),
            request_tx,
            tile_rx,
            runtime,
            tile_size,
        }
    }
}

impl TilesManager for Tiles {
    /// Attribution of the source this tile cache pulls images from. Typically,
    /// this should be displayed somewhere on the top of the map widget.
    fn attribution(&self) -> Attribution {
        self.attribution.clone()
    }

    /// Return a tile if already in cache, schedule a download otherwise.
    fn at(&mut self, tile_id: TileId) -> Option<Texture> {
        // Just take one at the time.
        match self.tile_rx.try_next() {
            Ok(Some((tile_id, tile))) => {
                self.cache.insert(tile_id, Some(tile));
            }
            Err(_) => {
                // Just ignore. It means that no new tile was downloaded.
            }
            Ok(None) => {
                log::error!("IO thread is dead")
            }
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

    fn tile_size(&self) -> u32 {
        self.tile_size
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    static TILE_ID: TileId = TileId {
        x: 1,
        y: 2,
        zoom: 3,
    };

    struct TestSource {
        base_url: String,
    }

    impl TestSource {
        pub fn new(base_url: String) -> Self {
            Self { base_url }
        }
    }

    impl TileSource for TestSource {
        fn tile_url(&self, tile_id: TileId) -> String {
            format!(
                "{}/{}/{}/{}.png",
                self.base_url, tile_id.zoom, tile_id.x, tile_id.y
            )
        }

        fn attribution(&self) -> Attribution {
            Attribution {
                text: "",
                url: "",
                logo_light: None,
                logo_dark: None,
            }
        }
    }

    /// Creates `mockito::Server` and function mapping `TileId` to this
    /// server's URL.
    fn mockito_server() -> (mockito::ServerGuard, TestSource) {
        let server = mockito::Server::new();
        let url = server.url();
        (server, TestSource::new(url))
    }

    #[test]
    fn download_single_tile() {
        let _ = env_logger::try_init();

        let (mut server, source) = mockito_server();
        let tile_mock = server
            .mock("GET", "/3/1/2.png")
            .with_body(include_bytes!("../assets/blank-255-tile.png"))
            .create();

        let mut tiles = Tiles::new(source, Context::default());

        // First query start the download, but it will always return None.
        assert!(tiles.at(TILE_ID).is_none());

        // Eventually it gets downloaded and become available in cache.
        while tiles.at(TILE_ID).is_none() {}

        tile_mock.assert();
    }

    fn assert_tile_is_empty_forever(tiles: &mut Tiles) {
        // Should be None now, and forever.
        assert!(tiles.at(TILE_ID).is_none());
        std::thread::sleep(Duration::from_secs(1));
        assert!(tiles.at(TILE_ID).is_none());
    }

    #[test]
    fn tile_is_empty_forever_if_http_returns_error() {
        let _ = env_logger::try_init();

        let (mut server, source) = mockito_server();
        let mut tiles = Tiles::new(source, Context::default());
        let tile_mock = server.mock("GET", "/3/1/2.png").with_status(404).create();

        assert_tile_is_empty_forever(&mut tiles);
        tile_mock.assert();
    }

    #[test]
    fn tile_is_empty_forever_if_http_returns_no_body() {
        let _ = env_logger::try_init();

        let (mut server, source) = mockito_server();
        let mut tiles = Tiles::new(source, Context::default());
        let tile_mock = server.mock("GET", "/3/1/2.png").create();

        assert_tile_is_empty_forever(&mut tiles);
        tile_mock.assert();
    }

    #[test]
    fn tile_is_empty_forever_if_http_returns_garbage() {
        let _ = env_logger::try_init();

        let (mut server, source) = mockito_server();
        let mut tiles = Tiles::new(source, Context::default());
        let tile_mock = server
            .mock("GET", "/3/1/2.png")
            .with_body("definitely not an image")
            .create();

        assert_tile_is_empty_forever(&mut tiles);
        tile_mock.assert();
    }

    struct GarbageSource;

    impl TileSource for GarbageSource {
        fn tile_url(&self, _: TileId) -> String {
            "totally invalid url".to_string()
        }

        fn attribution(&self) -> Attribution {
            Attribution {
                text: "",
                url: "",
                logo_light: None,
                logo_dark: None,
            }
        }
    }

    #[test]
    fn tile_is_empty_forever_if_http_can_not_even_connect() {
        let _ = env_logger::try_init();
        let mut tiles = Tiles::new(GarbageSource, Context::default());
        assert_tile_is_empty_forever(&mut tiles);
    }
}
