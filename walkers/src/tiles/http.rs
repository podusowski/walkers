use std::sync::Arc;

use bytes::Bytes;
use egui::Context;
use reqwest_middleware::ClientWithMiddleware;

use crate::io::Fetch;
use crate::io::http::http_client;
use crate::io::tiles_io::TilesIo;
use crate::sources::{Attribution, TileUrls};
use crate::style::Style;
use crate::{DemEncoding, HttpOptions, SourceData, StyledSources, TilePiece, TiledSource, Tiles};
use crate::{Stats, TileId};

/// Downloads the tiles via HTTP, and draws them with a style. A [`StyledSources`] with a single
/// [`HttpSource`]. It must persist between frames.
pub struct HttpTiles {
    styled: StyledSources,
    attribution: Attribution,
    tile_size: u32,
}

impl HttpTiles {
    /// Construct new [`Tiles`] with default [`HttpOptions`].
    pub fn new<S>(source: S, egui_ctx: Context) -> Self
    where
        S: TileUrls + Sync + Send + 'static,
    {
        Self::with_options(source, HttpOptions::default(), egui_ctx)
    }

    /// Construct new [`Tiles`] with supplied [`HttpOptions`].
    pub fn with_options<S>(source: S, http_options: HttpOptions, egui_ctx: Context) -> Self
    where
        S: TileUrls + Sync + Send + 'static,
    {
        Self::with_options_and_style(source, http_options, Default::default(), egui_ctx)
    }

    /// Construct new [`Tiles`] with supplied [`HttpOptions`] and [`Style`]. Style is relevant
    /// only for vector tile sources.
    pub fn with_options_and_style<S>(
        source: S,
        http_options: HttpOptions,
        style: Style,
        egui_ctx: Context,
    ) -> Self
    where
        S: TileUrls + Sync + Send + 'static,
    {
        let source = HttpSource::new(source, http_options, egui_ctx.clone());

        Self {
            attribution: source.attribution(),
            tile_size: source.tile_size(),
            styled: StyledSources::new(style, egui_ctx).with_tiled_source("", source),
        }
    }
}

/// Downloads tiles' data via HTTP, to be drawn by a [`crate::StyledSources`]. It must persist
/// between frames.
pub struct HttpSource {
    io: TilesIo,
    attribution: Attribution,
    tile_size: u32,
    max_zoom: u8,
}

impl HttpSource {
    pub fn new<S>(source: S, http_options: HttpOptions, egui_ctx: Context) -> Self
    where
        S: TileUrls + Sync + Send + 'static,
    {
        Self {
            attribution: source.attribution(),
            tile_size: source.tile_size(),
            max_zoom: source.max_zoom(),
            io: TilesIo::new(HttpFetch::new(source, http_options), egui_ctx),
        }
    }

    /// Tiles are elevation, packed as `encoding` says, for a `hillshade` style layer to draw.
    pub fn with_elevation(mut self, encoding: DemEncoding) -> Self {
        self.io = self.io.with_elevation(encoding);
        self
    }
}

impl TiledSource for HttpSource {
    fn data(&mut self, tile_id: TileId) -> Option<Arc<SourceData>> {
        self.io.data(tile_id)
    }

    fn attribution(&self) -> Attribution {
        self.attribution.clone()
    }

    fn tile_size(&self) -> u32 {
        self.tile_size
    }

    fn max_zoom(&self) -> u8 {
        self.max_zoom
    }

    fn stats(&self) -> Stats {
        self.io.stats()
    }
}

impl Tiles for HttpTiles {
    fn at(&mut self, tile_id: TileId) -> Option<TilePiece> {
        self.styled.single_source_at(tile_id)
    }

    fn attribution(&self) -> Attribution {
        self.attribution.clone()
    }

    fn tile_size(&self) -> u32 {
        self.tile_size
    }

    fn stats(&self) -> Stats {
        self.styled.stats()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HttpFetchError {
    #[error(transparent)]
    HttpMiddleware(#[from] reqwest_middleware::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
}

pub struct HttpFetch<S>
where
    S: TileUrls + Send + 'static,
{
    source: S,
    max_concurrency: usize,
    client: ClientWithMiddleware,
}

impl<S> HttpFetch<S>
where
    S: TileUrls + Sync + Send,
{
    pub fn new(source: S, http_options: HttpOptions) -> Self {
        Self {
            source,
            max_concurrency: http_options.max_parallel_downloads.0,
            client: http_client(&http_options),
        }
    }
}

impl<S> Fetch for HttpFetch<S>
where
    S: TileUrls + Sync + Send,
{
    type Error = HttpFetchError;

    async fn fetch(&self, tile_id: TileId) -> Result<Bytes, Self::Error> {
        let url = self.source.tile_url(tile_id);
        log::trace!("Downloading '{url}'.");
        let image = self.client.get(&url).send().await?;
        log::trace!("Downloaded '{}': {:?}.", url, image.status());
        Ok(image.error_for_status()?.bytes().await?)
    }

    fn max_concurrency(&self) -> usize {
        self.max_concurrency
    }
}

#[cfg(test)]
mod tests {
    use crate::MaxParallelDownloads;

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

    impl TileUrls for TestSource {
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

    /// Raster tiles need no style, so they must be drawn without the `vector` feature too.
    #[tokio::test]
    async fn raster_tile_is_drawn_on_the_canvas() {
        use crate::{Canvas, Layer, MapMemory, MercatorProjection, Projector, lon_lat};

        let _ = env_logger::try_init();

        let (server, source) = hypermocker_mock().await;
        server
            .anticipate("/0/0/0.png")
            .await
            .respond(include_bytes!("../../assets/blank-255-tile.png"))
            .await;

        let mut tiles = HttpTiles::new(source, Context::default());

        // The whole world is one tile at zoom 0.
        let mut memory = MapMemory::default();
        memory.set_zoom(0.).unwrap();
        let projector = Projector::new(
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(256.)),
            &MercatorProjection,
            &memory,
            lon_lat(0., 0.),
        );

        let canvas = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let mut canvas = Canvas::new();
                (&mut tiles).draw(&mut canvas, &projector);
                if !canvas.items.is_empty() {
                    break canvas;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("tile was never drawn");

        assert!(matches!(
            canvas.items.as_slice(),
            [crate::canvas::Item::Raster(_)]
        ));
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
            .respond(include_bytes!("../../assets/blank-255-tile.png"))
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
                "../../assets/blank-255-tile.png"
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

    impl TileUrls for GarbageSource {
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
