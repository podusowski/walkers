use crate::{
    sources::Attribution, tiles::interpolate_from_lower_zoom, Texture, TextureWithUv, TileId, Tiles,
};
use log::warn;
use lru::LruCache;
use std::path::{Path, PathBuf};

pub struct LocalTiles {
    path: PathBuf,
    egui_ctx: egui::Context,
    cache: LruCache<TileId, Texture>,
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

    fn load_and_cache(&mut self, tile_id: TileId) -> Option<Texture> {
        self.cache
            .try_get_or_insert(tile_id, || load(&self.path, tile_id, &self.egui_ctx))
            .inspect_err(|err| {
                warn!("Failed to load tile {:?}: {}", tile_id, err);
            })
            .cloned()
            .ok()
    }
}

impl Tiles for LocalTiles {
    fn at(&mut self, tile_id: TileId) -> Option<TextureWithUv> {
        let mut zoom_candidate = tile_id.zoom;

        loop {
            let (zoomed_tile_id, uv) = interpolate_from_lower_zoom(tile_id, zoom_candidate);

            if let Some(texture) = self.load_and_cache(zoomed_tile_id) {
                break Some(TextureWithUv {
                    texture: texture.clone(),
                    uv,
                });
            }

            // Keep zooming out until we find a donor or there is no more zoom levels.
            zoom_candidate = zoom_candidate.checked_sub(1)?;
        }
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
