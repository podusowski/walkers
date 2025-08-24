use std::path::{Path, PathBuf};

use crate::{sources::Attribution, Texture, TextureWithUv, Tiles};

pub struct LocalTiles {
    path: PathBuf,
    egui_ctx: egui::Context,
}

impl LocalTiles {
    pub fn new(path: impl AsRef<Path>, egui_ctx: egui::Context) -> Self {
        Self {
            path: path.as_ref().into(),
            egui_ctx,
        }
    }
}

impl Tiles for LocalTiles {
    fn at(&mut self, tile_id: crate::TileId) -> Option<crate::TextureWithUv> {
        load(&self.path, tile_id, &self.egui_ctx)
            .inspect_err(|err| {
                log::warn!("Failed to load tile {:?}: {}", tile_id, err);
            })
            .ok()
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
    tile_id: crate::TileId,
    egui_ctx: &egui::Context,
) -> Result<TextureWithUv, Box<dyn std::error::Error>> {
    let path = PathBuf::from_iter(&[
        tiles_dir.to_owned(),
        tile_id.zoom.to_string().into(),
        tile_id.x.to_string().into(),
        format!("{}.png", tile_id.y).into(),
    ]);
    let bytes = std::fs::read(path)?;
    Ok(TextureWithUv {
        texture: Texture::new(&bytes, egui_ctx)?,
        uv: egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
    })
}
