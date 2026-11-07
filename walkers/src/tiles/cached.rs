use egui::Context;

use super::interpolate_from_lower_zoom;
use crate::io::{Fetch, TileFactory, styling::StylingIo, tiles_io::TilesIo};
use crate::sources::Attribution;
use crate::{Stats, TileId, TilePiece, Tiles};

/// Tiles fetched and styled in the background, each kept in its own cache.
pub(crate) struct CachedTiles {
    fetched: TilesIo,
    styled: StylingIo,
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
            fetched: TilesIo::new(fetch, egui_ctx.clone()),
            styled: StylingIo::new(tile_factory, egui_ctx),
            attribution,
            tile_size,
            max_zoom,
        }
    }

    pub(crate) fn stats(&self) -> Stats {
        Stats {
            in_progress: self.fetched.stats().in_progress + self.styled.in_progress(),
        }
    }

    /// That tile if it is cached, otherwise the part of the nearest zoomed-out one covering
    /// the same ground.
    fn best_available(&mut self, tile_id: TileId) -> Option<TilePiece> {
        let mut zoom_candidate = tile_id.zoom;

        loop {
            let (zoomed_tile_id, uv) = interpolate_from_lower_zoom(tile_id, zoom_candidate);

            if let Some(Some(tile)) = self.styled.cache.get(&zoomed_tile_id) {
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
        self.fetched.put_single_fetched_tile_in_cache();
        self.styled.put_single_styled_tile_in_cache();

        if !tile_id.valid() {
            return None;
        }

        let tile_id_to_fetch = if tile_id.zoom > self.max_zoom {
            interpolate_from_lower_zoom(tile_id, self.max_zoom).0
        } else {
            tile_id
        };

        if !self.styled.knows(tile_id_to_fetch)
            && let Some(data) = self.fetched.data(tile_id_to_fetch)
        {
            self.styled.style(tile_id_to_fetch, data);
        }

        self.best_available(tile_id)
    }

    fn attribution(&self) -> Attribution {
        self.attribution.clone()
    }

    fn tile_size(&self) -> u32 {
        self.tile_size
    }
}
