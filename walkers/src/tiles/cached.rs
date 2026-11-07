use egui::Context;

use super::interpolate_from_lower_zoom;
use crate::io::{TileFactory, styling::StylingIo};
use crate::sources::Attribution;
use crate::{Stats, TileId, TilePiece, TiledSource, Tiles};

/// What a style makes of a source's tiles, styled in the background and kept in a cache.
pub(crate) struct StyledTiles {
    styled: StylingIo,
}

impl StyledTiles {
    pub(crate) fn new(
        tile_factory: impl TileFactory + Send + Sync + 'static,
        egui_ctx: Context,
    ) -> Self {
        Self {
            styled: StylingIo::new(tile_factory, egui_ctx),
        }
    }

    /// That tile if it is styled, otherwise the part of the nearest zoomed-out one covering the
    /// same ground. Styles the tile once `source` has its data.
    pub(crate) fn at<S: TiledSource + ?Sized>(
        &mut self,
        source: &mut S,
        tile_id: TileId,
    ) -> Option<TilePiece> {
        self.styled.put_single_styled_tile_in_cache();

        if !tile_id.valid() {
            return None;
        }

        let tile_id_to_style = if tile_id.zoom > source.max_zoom() {
            interpolate_from_lower_zoom(tile_id, source.max_zoom()).0
        } else {
            tile_id
        };

        if !self.styled.knows(tile_id_to_style)
            && let Some(data) = source.data(tile_id_to_style)
        {
            self.styled.style(tile_id_to_style, data);
        }

        self.best_available(tile_id)
    }

    /// How many tiles are being styled right now.
    pub(crate) fn in_progress(&self) -> usize {
        self.styled.in_progress()
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

/// A source and a style for it, seen as [`Tiles`].
pub(crate) struct SourceTiles<'a, S: TiledSource + ?Sized> {
    pub source: &'a mut S,
    pub styled: &'a mut StyledTiles,
}

impl<S: TiledSource + ?Sized> Tiles for SourceTiles<'_, S> {
    fn at(&mut self, tile_id: TileId) -> Option<TilePiece> {
        self.styled.at(self.source, tile_id)
    }

    fn attribution(&self) -> Attribution {
        self.source.attribution()
    }

    fn tile_size(&self) -> u32 {
        self.source.tile_size()
    }

    fn stats(&self) -> Stats {
        Stats {
            in_progress: self.source.stats().in_progress + self.styled.in_progress(),
        }
    }
}

/// One source, with the whole style drawn from it.
pub(crate) struct CachedTiles<S> {
    source: S,
    styled: StyledTiles,
}

impl<S: TiledSource> CachedTiles<S> {
    pub(crate) fn new(
        source: S,
        tile_factory: impl TileFactory + Send + Sync + 'static,
        egui_ctx: Context,
    ) -> Self {
        Self {
            source,
            styled: StyledTiles::new(tile_factory, egui_ctx),
        }
    }

    fn tiles(&mut self) -> SourceTiles<'_, S> {
        SourceTiles {
            source: &mut self.source,
            styled: &mut self.styled,
        }
    }
}

impl<S: TiledSource> Tiles for CachedTiles<S> {
    fn at(&mut self, tile_id: TileId) -> Option<TilePiece> {
        self.tiles().at(tile_id)
    }

    fn attribution(&self) -> Attribution {
        self.source.attribution()
    }

    fn tile_size(&self) -> u32 {
        self.source.tile_size()
    }

    fn stats(&self) -> Stats {
        Stats {
            in_progress: self.source.stats().in_progress + self.styled.in_progress(),
        }
    }
}
