use crate::{sources::Attribution, Texture, TextureWithUv, TileId, Tiles};
use egui::{pos2, Rect};
use lru::LruCache;
use pmtiles::{AsyncPmTilesReader, TileCoord};
use std::{
    io::Read as _,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Clone)]
enum CachedTexture {
    Valid(Texture),
    Invalid,
}

/// https://docs.protomaps.com/guide/getting-started
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
                    log::warn!("Failed to load tile {:?}: {}", tile_id, err);
                    CachedTexture::Invalid
                }
            })
            .clone()
    }
}

impl Tiles for PmTiles {
    fn at(&mut self, tile_id: TileId) -> Option<TextureWithUv> {
        match self.load_and_cache(tile_id) {
            CachedTexture::Valid(texture) => Some(TextureWithUv::new(
                texture.clone(),
                Rect::from_min_size(pos2(0.0, 0.0), egui::Vec2::splat(1.0)),
            )),
            CachedTexture::Invalid => None,
        }
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
        512
    }
}

#[derive(Debug, Error)]
enum PmTilesError {
    #[error("Tile not found")]
    TileNotFound,
    #[error(transparent)]
    Other(#[from] pmtiles::PmtError),
}

fn load(path: &Path, tile_id: TileId) -> Result<Texture, Box<dyn std::error::Error>> {
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

    let decompressed = decompress(&bytes);
    Ok(Texture::from_mvt(&decompressed)?)
}

/// Decompress the tile.
///
/// This function assumes the input is gzip compressed data, but this might not always be the case.
/// You can use `pmtiles info <file>` to check the compression type.
fn decompress(data: &[u8]) -> Vec<u8> {
    let mut decoder = flate2::read::GzDecoder::new(data);
    let mut buf = Vec::new();
    decoder
        .read_to_end(&mut buf)
        .expect("Failed to decompress tile");
    buf
}
