use egui::Context;

use crate::TileId;
use crate::download::{HttpFetch, HttpOptions};
use crate::io::tiles_io::TilesIo;
use crate::sources::{Attribution, TileSource};
use crate::tiles::interpolate_from_lower_zoom;
use crate::{TilePiece, Tiles};

/// Downloads the tiles via HTTP. It must persist between frames.
pub struct HttpTiles {
    attribution: Attribution,
    loader: TilesIo,
    tile_size: u32,
    max_zoom: u8,
}

impl HttpTiles {
    /// Construct new [`Tiles`] with default [`HttpOptions`].
    pub fn new<S>(source: S, egui_ctx: Context) -> Self
    where
        S: TileSource + Sync + Send + 'static,
    {
        Self::with_options(source, HttpOptions::default(), egui_ctx)
    }

    /// Construct new [`Tiles`] with supplied [`HttpOptions`].
    pub fn with_options<S>(source: S, http_options: HttpOptions, egui_ctx: Context) -> Self
    where
        S: TileSource + Sync + Send + 'static,
    {
        let attribution = source.attribution();
        let tile_size = source.tile_size();
        let max_zoom = source.max_zoom();
        let fetch = HttpFetch::new(source, http_options);

        Self {
            attribution,
            loader: TilesIo::new(fetch, egui_ctx),
            tile_size,
            max_zoom,
        }
    }

    pub fn stats(&self) -> HttpStats {
        if let Ok(http_stats) = self.loader.stats.lock() {
            http_stats.clone()
        } else {
            // I really do not want this to return a Result.
            HttpStats::default()
        }
    }

    /// Get at tile, or interpolate it from lower zoom levels. This function does not start any
    /// downloads.
    fn get_from_cache_or_interpolate(&mut self, tile_id: TileId) -> Option<TilePiece> {
        let mut zoom_candidate = tile_id.zoom;

        loop {
            let (zoomed_tile_id, uv) = interpolate_from_lower_zoom(tile_id, zoom_candidate);

            if let Some(Some(texture)) = self.loader.cache.get(&zoomed_tile_id) {
                break Some(TilePiece {
                    texture: texture.clone(),
                    uv,
                });
            }

            // Keep zooming out until we find a donor or there is no more zoom levels.
            zoom_candidate = zoom_candidate.checked_sub(1)?;
        }
    }
}

#[derive(Clone, Default)]
pub struct HttpStats {
    /// Number of tiles that are currently being downloaded.
    pub in_progress: usize,
}

impl Tiles for HttpTiles {
    /// Attribution of the source this tile cache pulls images from. Typically,
    /// this should be displayed somewhere on the top of the map widget.
    fn attribution(&self) -> Attribution {
        self.attribution.clone()
    }

    /// Return a tile if already in cache, schedule a download otherwise.
    fn at(&mut self, tile_id: TileId) -> Option<TilePiece> {
        self.loader.put_single_downloaded_tile_in_cache();

        if !tile_id.valid() {
            return None;
        }

        let tile_id_to_download = if tile_id.zoom > self.max_zoom {
            interpolate_from_lower_zoom(tile_id, self.max_zoom).0
        } else {
            tile_id
        };

        self.loader.make_sure_is_downloaded(tile_id_to_download);
        self.get_from_cache_or_interpolate(tile_id)
    }

    fn tile_size(&self) -> u32 {
        self.tile_size
    }
}

#[cfg(test)]
mod tests {
    use crate::download::MaxParallelDownloads;

    use super::*;
    use hypermocker::{
        Bytes, StatusCode,
        hyper::header::{self, HeaderValue},
    };
    use std::time::Duration;

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

    /// Creates [`hypermocker::Mock`], and function mapping `TileId` to its URL.
    async fn hypermocker_mock() -> (hypermocker::Server, TestSource) {
        let server = hypermocker::Server::bind().await;
        let url = format!("http://localhost:{}", server.port());
        (server, TestSource::new(url))
    }

    async fn assert_tile_to_become_available_eventually(tiles: &mut HttpTiles, tile_id: TileId) {
        log::info!("Waiting for {tile_id:?} to become available.");
        while tiles.at(tile_id).is_none() {
            // Need to yield to the runtime for things to move.
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn download_single_tile() {
        let _ = env_logger::try_init();

        let (server, source) = hypermocker_mock().await;
        let mut anticipated = server.anticipate("/3/1/2.png").await;

        let mut tiles = HttpTiles::new(source, Context::default());

        // First query start the download, but it will always return None.
        assert!(tiles.at(TILE_ID).is_none());

        let request = anticipated.expect().await;
        assert_eq!(
            request.headers().get(header::USER_AGENT),
            Some(&HeaderValue::from_static(concat!(
                "walkers",
                "/",
                env!("CARGO_PKG_VERSION"),
            )))
        );

        // Eventually it gets downloaded and become available in cache.
        anticipated
            .respond(include_bytes!("../assets/blank-255-tile.png"))
            .await;
        assert_tile_to_become_available_eventually(&mut tiles, TILE_ID).await;
    }

    #[tokio::test]
    async fn download_is_not_started_when_tile_is_invalid() {
        let _ = env_logger::try_init();

        let (_server, source) = hypermocker_mock().await;
        let mut tiles = HttpTiles::new(source, Context::default());

        let invalid_tile_id = TileId {
            x: 2,
            y: 2,
            zoom: 0, // There only one tile at zoom 0.
        };

        assert!(tiles.at(invalid_tile_id).is_none());

        // Make sure it does not come.
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    #[tokio::test]
    async fn custom_user_agent_header() {
        let _ = env_logger::try_init();

        let (server, source) = hypermocker_mock().await;
        let mut anticipated = server.anticipate("/3/1/2.png").await;

        let mut tiles = HttpTiles::with_options(
            source,
            HttpOptions {
                user_agent: Some(crate::HeaderValue::from_static("MyApp")),
                ..Default::default()
            },
            Context::default(),
        );

        // Initiate the download.
        tiles.at(TILE_ID);

        let request = anticipated.expect().await;
        assert_eq!(
            request.headers().get(header::USER_AGENT),
            Some(&HeaderValue::from_static("MyApp"))
        );
    }

    #[tokio::test]
    async fn by_default_there_can_be_6_parallel_downloads_at_most() {
        let _ = env_logger::try_init();

        there_can_be_x_parallel_downloads_at_most(6, HttpOptions::default()).await;
    }

    #[tokio::test]
    async fn there_can_be_10_parallel_downloads_at_most() {
        let _ = env_logger::try_init();

        there_can_be_x_parallel_downloads_at_most(
            10,
            HttpOptions {
                max_parallel_downloads:
                    MaxParallelDownloads::value_manually_confirmed_with_provider_limits(10),
                ..Default::default()
            },
        )
        .await;
    }

    async fn there_can_be_x_parallel_downloads_at_most(x: u32, http_options: HttpOptions) {
        let _ = env_logger::try_init();

        let (server, source) = hypermocker_mock().await;
        let mut tiles = HttpTiles::with_options(source, http_options, Context::default());

        // First download is started immediately.
        let mut first = server.anticipate("/3/1/2.png".to_string()).await;
        assert!(tiles.at(TILE_ID).is_none());
        first.expect().await;

        // Rest of the downloads are started right away too, but they remain active.
        let mut active = Vec::new();
        for x in 0..x - 1 {
            let tile_id = TileId { x, y: 1, zoom: 10 };
            let mut request = server.anticipate(format!("/10/{}/1.png", tile_id.x)).await;
            assert!(tiles.at(tile_id).is_none());
            request.expect().await;
            active.push(request);
        }

        // Last download is NOT started, because we are at the limit of concurrent downloads.
        assert!(
            tiles
                .at(TileId {
                    x: 99,
                    y: 99,
                    zoom: 10
                })
                .is_none()
        );

        // Make sure it does not come.
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Last download will start as soon as one of the previous ones are responded to.
        let mut awaiting_request = server.anticipate("/10/99/99.png".to_string()).await;

        first
            .respond(Bytes::from_static(include_bytes!(
                "../assets/blank-255-tile.png"
            )))
            .await;

        awaiting_request.expect().await;
    }

    async fn assert_tile_is_empty_forever(tiles: &mut HttpTiles) {
        // Should be None now, and forever.
        assert!(tiles.at(TILE_ID).is_none());
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert!(tiles.at(TILE_ID).is_none());
    }

    #[tokio::test]
    async fn tile_is_empty_forever_if_http_returns_error() {
        let _ = env_logger::try_init();

        let (server, source) = hypermocker_mock().await;
        let mut tiles = HttpTiles::new(source, Context::default());
        server
            .anticipate("/3/1/2.png")
            .await
            .respond_with_status(StatusCode::NOT_FOUND)
            .await;

        assert_tile_is_empty_forever(&mut tiles).await;
    }

    #[tokio::test]
    async fn tile_is_empty_forever_if_http_returns_no_body() {
        let _ = env_logger::try_init();

        let (server, source) = hypermocker_mock().await;
        let mut tiles = HttpTiles::new(source, Context::default());
        server
            .anticipate("/3/1/2.png")
            .await
            .respond_with_status(StatusCode::OK)
            .await;

        assert_tile_is_empty_forever(&mut tiles).await;
    }

    #[tokio::test]
    async fn tile_is_empty_forever_if_http_returns_garbage() {
        let _ = env_logger::try_init();

        let (server, source) = hypermocker_mock().await;
        let mut tiles = HttpTiles::new(source, Context::default());
        server
            .anticipate("/3/1/2.png")
            .await
            .respond("definitely not an image")
            .await;

        assert_tile_is_empty_forever(&mut tiles).await;
    }

    /// Tile source, which gives invalid urls.
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

    #[tokio::test]
    async fn tile_is_empty_forever_if_http_can_not_even_connect() {
        let _ = env_logger::try_init();
        let mut tiles = HttpTiles::new(GarbageSource, Context::default());
        assert_tile_is_empty_forever(&mut tiles).await;
    }
}
