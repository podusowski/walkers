use crate::{
    Texture, TextureWithUv, TileId, Tiles, download::Fetch, sources::Attribution,
    tiles::interpolate_from_lower_zoom,
};
use bytes::Bytes;
use lru::LruCache;
use pmtiles::{AsyncPmTilesReader, TileCoord};
use std::{
    io::{self, Read as _},
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Clone)]
enum CachedTexture {
    Valid(Texture),
    Invalid,
}

/// Provides tiles from a local PMTiles file.
///
/// <https://docs.protomaps.com/guide/getting-started>
pub struct PmTiles {
    path: PathBuf,
    cache: LruCache<TileId, CachedTexture>,
}

impl PmTiles {
    pub fn new(path: impl AsRef<Path>) -> Self {
        // Just arbitrary value which seemed right.
        #[allow(clippy::unwrap_used)]
        let cache_size = std::num::NonZeroUsize::new(256).unwrap();

        Self {
            path: path.as_ref().into(),
            cache: LruCache::new(cache_size),
        }
    }

    fn load_and_cache(&mut self, tile_id: TileId) -> CachedTexture {
        self.cache
            .get_or_insert(tile_id, || match load(&self.path, tile_id) {
                Ok(texture) => CachedTexture::Valid(texture),
                Err(err) => {
                    log::warn!("Failed to load tile {tile_id:?}: {err}");
                    CachedTexture::Invalid
                }
            })
            .clone()
    }
}

impl Tiles for PmTiles {
    fn at(&mut self, tile_id: TileId) -> Option<TextureWithUv> {
        (0..=tile_id.zoom).rev().find_map(|zoom_candidate| {
            let (donor_tile_id, uv) = interpolate_from_lower_zoom(tile_id, zoom_candidate);
            match self.load_and_cache(donor_tile_id) {
                CachedTexture::Valid(texture) => Some(TextureWithUv::new(texture.clone(), uv)),
                CachedTexture::Invalid => None,
            }
        })
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
    async fn new(path: &Path) -> Self {
        Self {
            path: path.to_owned(),
        }
    }
}

impl Fetch for PmTilesFetch {
    type Error = PmTilesError;

    async fn fetch(&self, tile_id: TileId) -> Result<Bytes, Self::Error> {
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

fn load(path: &Path, tile_id: TileId) -> Result<Texture, Box<dyn std::error::Error>> {
    // TODO: Yes, that's heavy.
    let bytes = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let reader = AsyncPmTilesReader::new_with_path(path).await?;
            reader
                .get_tile(TileCoord::new(tile_id.zoom, tile_id.x, tile_id.y)?)
                .await?
                .ok_or(PmTilesError::TileNotFound)
        })?;

    let decompressed = decompress(&bytes)?;
    Ok(Texture::from_mvt(&decompressed)?)
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
