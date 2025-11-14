use crate::{
    TileId, TilePiece, Tiles, download::Fetch, io::tiles_io::TilesIo, sources::Attribution,
    tiles::interpolate_from_lower_zoom,
};
use bytes::Bytes;
use pmtiles::{AsyncPmTilesReader, TileCoord};
use std::{
    io::{self, Read as _},
    path::{Path, PathBuf},
};
use thiserror::Error;

/// Provides tiles from a local PMTiles file.
///
/// <https://docs.protomaps.com/guide/getting-started>
pub struct PmTiles {
    tiles_io: TilesIo,
}

impl PmTiles {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            tiles_io: TilesIo::new(PmTilesFetch::new(path.as_ref()), egui::Context::default()),
        }
    }

    /// Get at tile, or interpolate it from lower zoom levels. This function does not start any
    /// downloads.
    fn get_from_cache_or_interpolate(&mut self, tile_id: TileId) -> Option<TilePiece> {
        let mut zoom_candidate = tile_id.zoom;

        loop {
            let (zoomed_tile_id, uv) = interpolate_from_lower_zoom(tile_id, zoom_candidate);

            if let Some(Some(texture)) = self.tiles_io.cache.get(&zoomed_tile_id) {
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

impl Tiles for PmTiles {
    fn at(&mut self, tile_id: TileId) -> Option<TilePiece> {
        self.tiles_io.put_single_downloaded_tile_in_cache();

        if !tile_id.valid() {
            return None;
        }

        let tile_id_to_download = if tile_id.zoom > 16 {
            interpolate_from_lower_zoom(tile_id, 16).0
        } else {
            tile_id
        };

        self.tiles_io.make_sure_is_downloaded(tile_id_to_download);
        self.get_from_cache_or_interpolate(tile_id)
    }

    fn attribution(&self) -> Attribution {
        Attribution {
            text: "PMTiles",
            url: "",
            logo_light: None,
            logo_dark: None,
        }
    }

    fn tile_size(&self) -> u32 {
        // Vector tiles can be rendered at any size. Effectively this means that the lower the
        // tile, the more details are visible.
        1024
    }
}

#[derive(Debug, Error)]
enum PmTilesError {
    #[error("Tile not found")]
    TileNotFound,
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
        let bytes = reader
            .get_tile(TileCoord::new(tile_id.zoom, tile_id.x, tile_id.y)?)
            .await?
            .ok_or(PmTilesError::TileNotFound)?;

        Ok(decompress(&bytes)?.into())
    }

    fn max_concurrency(&self) -> usize {
        // Just an arbitrary value. Probably should be aligned to the number of CPU cores as most
        // of the vector tile loading work is CPU-bound. Number of threads for Tokio runtime should
        // follow this value as well.
        6
    }
}

/// Decompress the tile.
///
/// This function assumes the input is gzip compressed data, but this might not always be the case.
/// You can use `pmtiles info <file>` to check the compression type.
fn decompress(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut decoder = flate2::read::GzDecoder::new(data);
    let mut buf = Vec::new();
    decoder.read_to_end(&mut buf)?;
    Ok(buf)
}
