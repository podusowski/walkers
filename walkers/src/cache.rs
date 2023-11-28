use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::path::PathBuf;

use crate::mercator::TileId;
use crate::providers::TileSource;

/// Trait for custom cache implementations
pub trait TileCache {
    type Error;

    fn read<S>(&mut self, provider: &S, tile_id: TileId) -> Result<Option<Vec<u8>>, Self::Error>
    where
        S: TileSource + Hash;

    fn write<S>(&mut self, provider: &S, tile_id: TileId, data: &[u8]) -> Result<(), Self::Error>
    where
        S: TileSource + Hash;
}

/// A cache implementation that does nothing. Use this if you cannot or do not want to use another
/// cache implementation, for instance on WebAssembly.
#[derive(Debug, Clone)]
pub struct NoopCache {}

impl TileCache for NoopCache {
    type Error = String;

    fn read<S>(&mut self, _provider: &S, _tile_id: TileId) -> Result<Option<Vec<u8>>, Self::Error>
    where
        S: TileSource + Hash,
    {
        Ok(None)
    }

    fn write<S>(&mut self, _provider: &S, _tile_id: TileId, _data: &[u8]) -> Result<(), Self::Error>
    where
        S: TileSource + Hash,
    {
        Ok(())
    }
}

/// A cache implementation storing tiles in a directory on-disk. Can safely be cloned and reused
/// for different tile providers.
#[derive(Debug, Clone)]
pub struct DiskTileCache {
    directory: PathBuf,
}

impl DiskTileCache {
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        Self {
            directory: path.into(),
        }
    }

    fn cache_file_path<S>(&self, provider: &S, tile_id: TileId) -> PathBuf
    where
        S: TileSource + Hash,
    {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        provider.hash(&mut hasher);
        let provider_hash = hasher.finish();

        self.directory
            .join(format!("{:016x}", provider_hash))
            .join(format!("{}_{}_{}", tile_id.zoom, tile_id.x, tile_id.y))
    }
}

impl TileCache for DiskTileCache {
    type Error = std::io::Error;

    fn read<S>(&mut self, provider: &S, tile_id: TileId) -> Result<Option<Vec<u8>>, Self::Error>
    where
        S: TileSource + Hash,
    {
        let path = self.cache_file_path(provider, tile_id);
        if path.exists() {
            let mut f = File::open(path)?;
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer)?;
            Ok(Some(buffer))
        } else {
            Ok(None)
        }
    }

    fn write<S>(&mut self, provider: &S, tile_id: TileId, data: &[u8]) -> Result<(), Self::Error>
    where
        S: TileSource + Hash,
    {
        let path = self.cache_file_path(provider, tile_id);

        // This should always succeed, given we created the path above
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }

        let mut f = File::create(path)?;
        f.write_all(data)?;

        Ok(())
    }
}
