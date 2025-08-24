use std::path::{Path, PathBuf};

use crate::Tiles;

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

    fn attribution(&self) -> crate::sources::Attribution {
        todo!()
    }

    fn tile_size(&self) -> u32 {
        256
    }
}
