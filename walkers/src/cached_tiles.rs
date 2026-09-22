use egui::Context;

use crate::io::{Fetch, TileFactory, tiles_io::TilesIo};
use crate::sources::Attribution;
use crate::tiles::interpolate_from_lower_zoom;
use crate::{Stats, TileId, TilePiece, Tiles};

/// Tiles fetched in the background and kept in a cache.
pub(crate) struct CachedTiles {
    io: TilesIo,
    attribution: Attribution,
    tile_size: u32,
    max_zoom: u8,
}

impl CachedTiles {
    pub(crate) fn new(
        fetch: impl Fetch + Send + Sync + 'static,
        tile_factory: impl TileFactory + Send + Sync + 'static,
        attribution: Attribution,
        tile_size: u32,
        max_zoom: u8,
        egui_ctx: Context,
    ) -> Self {
        Self {
            io: TilesIo::new(fetch, tile_factory, egui_ctx.clone()),
            attribution,
            tile_size,
            max_zoom,
        }
    }

    pub(crate) fn stats(&self) -> Stats {
        self.io.stats()
    }

    /// That tile if it is cached, otherwise the part of the nearest zoomed-out one covering
    /// the same ground.
    fn best_available(&mut self, tile_id: TileId) -> Option<TilePiece> {
        let mut zoom_candidate = tile_id.zoom;

        loop {
            let (zoomed_tile_id, uv) = interpolate_from_lower_zoom(tile_id, zoom_candidate);

            if let Some(Some(tile)) = self.io.cache.get(&zoomed_tile_id) {
                break Some(TilePiece {
                    tile: tile.clone(),
                    uv,
                });
            }

            // Keep zooming out until we find a donor or there is no more zoom levels.
            zoom_candidate = zoom_candidate.checked_sub(1)?;
        }
    }
}

impl Tiles for CachedTiles {
    fn at(&mut self, tile_id: TileId) -> Option<TilePiece> {
        self.io.put_single_fetched_tile_in_cache();

        if !tile_id.valid() {
            return None;
        }

        let tile_id_to_fetch = if tile_id.zoom > self.max_zoom {
            interpolate_from_lower_zoom(tile_id, self.max_zoom).0
        } else {
            tile_id
        };

        self.io.make_sure_is_fetched(tile_id_to_fetch);
        self.best_available(tile_id)
    }

    fn attribution(&self) -> Attribution {
        self.attribution.clone()
    }

    fn tile_size(&self) -> u32 {
        self.tile_size
    }
}
