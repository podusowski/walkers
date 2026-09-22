use crate::{
    TileId, TilePiece, Tiles, cached_tiles::CachedTiles, io::Fetch, sources::Attribution,
    style::Style, tiles::EguiTileFactory,
};
use bytes::Bytes;
use egui::Context;
use pmtiles::{AsyncPmTilesReader, TileCoord};
use std::{
    io::{self},
    path::{Path, PathBuf},
};
use thiserror::Error;

/// What a tile is rendered for, unless asked for something else. Larger means fewer tiles
/// covering the map, at less detail.
const DEFAULT_TILE_SIZE: u32 = 1024;

const DEFAULT_MAX_ZOOM: u8 = 15;

/// Provides tiles from a local PMTiles file.
///
/// <https://docs.protomaps.com/guide/getting-started>
pub struct PmTiles(CachedTiles);

impl PmTiles {
    pub fn new(path: impl AsRef<Path>, egui_ctx: Context) -> Self {
        Self::with_style(path, Style::default(), egui_ctx)
    }

    /// Construct new [`PmTiles`] with [`Style`]. Style is relevant only for vector tile
    /// sources.
    pub fn with_style(path: impl AsRef<Path>, style: Style, egui_ctx: Context) -> Self {
        Self::with_style_and_tile_size(path, style, DEFAULT_TILE_SIZE, egui_ctx)
    }

    /// Tiles are rendered for `tile_size`, which is the size they are drawn at when the map is
    /// at one of their zoom levels. Everything a style measures in pixels is measured against
    /// it, so it has to be known before the first tile is decoded.
    pub fn with_style_and_tile_size(
        path: impl AsRef<Path>,
        style: Style,
        tile_size: u32,
        egui_ctx: Context,
    ) -> Self {
        Self(CachedTiles::new(
            PmTilesFetch::new(path.as_ref()),
            EguiTileFactory::new(egui_ctx.clone(), style, tile_size),
            Attribution {
                text: "PMTiles",
                url: "",
                logo_light: None,
                logo_dark: None,
            },
            tile_size,
            DEFAULT_MAX_ZOOM,
            egui_ctx,
        ))
    }
}

impl Tiles for PmTiles {
    fn at(&mut self, tile_id: TileId) -> Option<TilePiece> {
        self.0.at(tile_id)
    }

    fn attribution(&self) -> Attribution {
        self.0.attribution()
    }

    fn tile_size(&self) -> u32 {
        self.0.tile_size()
    }
}

#[derive(Debug, Error)]
enum PmTilesError {
    #[error("Tile {0:?} not found in pmtiles file.")]
    TileNotFound(TileId),
    #[error(transparent)]
    Decompression(#[from] io::Error),
    #[error(transparent)]
    Other(#[from] pmtiles::PmtError),
}

struct PmTilesFetch {
    path: PathBuf,
}

impl PmTilesFetch {
    fn new(path: &Path) -> Self {
        Self {
            path: path.to_owned(),
        }
    }
}

impl Fetch for PmTilesFetch {
    type Error = PmTilesError;

    async fn fetch(&self, tile_id: TileId) -> Result<Bytes, Self::Error> {
        // TODO: Avoid reopening the file every time.
        let reader = AsyncPmTilesReader::new_with_path(self.path.to_owned()).await?;

        reader
            .get_tile_decompressed(TileCoord::new(tile_id.zoom, tile_id.x, tile_id.y)?)
            .await?
            .ok_or(PmTilesError::TileNotFound(tile_id))
    }

    fn max_concurrency(&self) -> usize {
        // Just an arbitrary value. Probably should be aligned to the number of CPU cores as most
        // of the vector tile loading work is CPU-bound. Number of threads for Tokio runtime should
        // follow this value as well.
        6
    }
}
