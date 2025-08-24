use std::path::{Path, PathBuf};

use crate::{sources::Attribution, Tiles};

pub struct LocalTiles {
    path: PathBuf,
}

impl LocalTiles {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().into(),
        }
    }
}

impl Tiles for LocalTiles {
    fn at(&mut self, tile_id: crate::TileId) -> Option<crate::TextureWithUv> {
        todo!()
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
