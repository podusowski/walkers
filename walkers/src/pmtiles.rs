use crate::{
    io::Runtime, sources::Attribution, tiles::interpolate_from_lower_zoom, Texture, TextureWithUv,
    TileId, Tiles,
};
use flate2::read::ZlibDecoder;
use log::trace;
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
    egui_ctx: egui::Context,
    cache: LruCache<TileId, CachedTexture>,
}

impl PmTiles {
    pub fn new(path: impl AsRef<Path>, egui_ctx: egui::Context) -> Self {
        // Just arbitrary value which seemed right.
        #[allow(clippy::unwrap_used)]
        let cache_size = std::num::NonZeroUsize::new(256).unwrap();

        Self {
            path: path.as_ref().into(),
            egui_ctx,
            cache: LruCache::new(cache_size),
        }
    }

    fn load_and_cache(&mut self, tile_id: TileId) -> CachedTexture {
        self.cache
            .get_or_insert(tile_id, || {
                match load(&self.path, tile_id, &self.egui_ctx) {
                    Ok(texture) => CachedTexture::Valid(texture),
                    Err(err) => {
                        log::warn!("Failed to load tile {:?}: {}", tile_id, err);
                        CachedTexture::Invalid
                    }
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
        4096
    }
}

#[derive(Debug, Error)]
#[error("PMTiles error")]
struct PmTilesError;

fn load(
    path: &Path,
    tile_id: TileId,
    egui_ctx: &egui::Context,
) -> Result<Texture, Box<dyn std::error::Error>> {
    let bytes = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let reader = AsyncPmTilesReader::new_with_path(path).await.unwrap();
            reader
                .get_tile(TileCoord::new(tile_id.zoom, tile_id.x, tile_id.y).unwrap())
                .await
                .unwrap()
                .ok_or(PmTilesError)
        })?;

    let decompressed = decompress(&bytes);
    Ok(crate::mvt::render(&decompressed, egui_ctx)?)
}

/// Decode the tile.
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
