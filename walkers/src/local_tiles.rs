use crate::{
    Texture, TextureWithUv, TileId, Tiles, sources::Attribution, tiles::interpolate_from_lower_zoom,
};
use log::trace;
use lru::LruCache;
use std::path::{Path, PathBuf};

#[derive(Clone)]
enum CachedTexture {
    Valid(Texture),
    Invalid,
}

/// Uses local directory as tile source.
pub struct LocalTiles {
    path: PathBuf,
    egui_ctx: egui::Context,
    cache: LruCache<TileId, CachedTexture>,
}

impl LocalTiles {
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
                        trace!("Failed to load tile {tile_id:?}: {err}");
                        CachedTexture::Invalid
                    }
                }
            })
            .clone()
    }
}

impl Tiles for LocalTiles {
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
            text: "Local tiles",
            url: "",
            logo_light: None,
            logo_dark: None,
        }
    }

    fn tile_size(&self) -> u32 {
        256
    }
}

fn load(
    tiles_dir: &Path,
    tile_id: TileId,
    egui_ctx: &egui::Context,
) -> Result<Texture, Box<dyn std::error::Error>> {
    let path = PathBuf::from_iter(&[
        tiles_dir.to_owned(),
        tile_id.zoom.to_string().into(),
        tile_id.x.to_string().into(),
        format!("{}.png", tile_id.y).into(),
    ]);
    let bytes = std::fs::read(path)?;
    Ok(Texture::new(&bytes, egui_ctx)?)
}
