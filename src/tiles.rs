use std::collections::{hash_map::Entry, HashMap};

use egui::{pos2, Color32, Context, Mesh, Pos2, Rect, Vec2};
use egui::{ColorImage, TextureHandle};
use futures::channel::mpsc::{channel, Receiver, Sender, TrySendError};
use image::ImageError;
use lru::LruCache;

use crate::{
    download::{download_continuously, HttpOptions, MAX_PARALLEL_DOWNLOADS},
    io::Runtime,
    sources::{Attribution, TileSource},
    units::Pixel,
};

pub(crate) fn rect(screen_position: Pos2, tile_size: f64) -> Rect {
    Rect::from_min_size(screen_position, Vec2::splat(tile_size as f32))
}

#[derive(Clone)]
pub struct Texture(TextureHandle);

impl Texture {
    pub fn new(image: &[u8], ctx: &Context) -> Result<Self, ImageError> {
        let image = image::load_from_memory(image)?.to_rgba8();
        let pixels = image.as_flat_samples();
        let image = ColorImage::from_rgba_unmultiplied(
            [image.width() as _, image.height() as _],
            pixels.as_slice(),
        );

        Ok(Self::from_color_image(image, ctx))
    }

    /// Load the texture from egui's [`ColorImage`].
    pub fn from_color_image(color_image: ColorImage, ctx: &Context) -> Self {
        Self(ctx.load_texture("image", color_image, Default::default()))
    }

    pub(crate) fn mesh_with_uv(&self, screen_position: Pos2, tile_size: f64, uv: Rect) -> Mesh {
        self.mesh_with_rect_and_uv(rect(screen_position, tile_size), uv)
    }

    pub(crate) fn mesh_with_rect_and_uv(&self, rect: Rect, uv: Rect) -> Mesh {
        let mut mesh = Mesh::with_texture(self.0.id());
        mesh.add_rect_with_uv(rect, uv, Color32::WHITE);
        mesh
    }

    pub(crate) fn size(&self) -> Vec2 {
        self.0.size_vec2()
    }

    pub(crate) fn mesh_with_rect(&self, rect: Rect) -> Mesh {
        let mut mesh = Mesh::with_texture(self.0.id());
        mesh.add_rect_with_uv(
            rect,
            Rect::from_min_max(pos2(0., 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        mesh
    }
}

/// Texture with UV coordinates.
pub struct TextureWithUv {
    pub texture: Texture,
    pub uv: Rect,
}

pub trait Tiles {
    fn at(&mut self, tile_id: TileId) -> Option<TextureWithUv>;
    fn attribution(&self) -> Attribution;
    fn tile_size(&self) -> u32;
}

/// Downloads the tiles via HTTP. It must persist between frames.
pub struct HttpTiles {
    attribution: Attribution,

    cache: LruCache<TileId, Option<Texture>>,

    /// Tiles to be downloaded by the IO thread.
    request_tx: Sender<TileId>,

    /// Tiles that got downloaded and should be put in the cache.
    tile_rx: Receiver<(TileId, Texture)>,

    #[allow(dead_code)] // Significant Drop
    runtime: Runtime,

    tile_size: u32,

    max_zoom: u8,
}

impl HttpTiles {
    /// Construct new [`Tiles`] with default [`HttpOptions`].
    pub fn new<S>(source: S, egui_ctx: Context) -> Self
    where
        S: TileSource + Send + 'static,
    {
        Self::with_options(source, HttpOptions::default(), egui_ctx)
    }

    /// Construct new [`Tiles`] with supplied [`HttpOptions`].
    pub fn with_options<S>(source: S, http_options: HttpOptions, egui_ctx: Context) -> Self
    where
        S: TileSource + Send + 'static,
    {
        // This ensures that newer requests are prioritized.
        let channel_size = MAX_PARALLEL_DOWNLOADS;

        let (request_tx, request_rx) = channel(channel_size);
        let (tile_tx, tile_rx) = channel(channel_size);
        let attribution = source.attribution();
        let tile_size = source.tile_size();
        let max_zoom = source.max_zoom();

        let runtime = Runtime::new(download_continuously(
            source,
            http_options,
            request_rx,
            tile_tx,
            egui_ctx,
        ));

        // Just arbitrary value which seemed right.
        #[allow(clippy::unwrap_used)]
        let cache_size = std::num::NonZeroUsize::new(256).unwrap();

        Self {
            attribution,
            cache: LruCache::new(cache_size),
            request_tx,
            tile_rx,
            runtime,
            tile_size,
            max_zoom,
        }
    }

    fn put_single_downloaded_tile_in_cache(&mut self) {
        // This is called every frame, so take just one at the time.
        match self.tile_rx.try_next() {
            Ok(Some((tile_id, tile))) => {
                self.cache.put(tile_id, Some(tile));
            }
            Err(_) => {
                // Just ignore. It means that no new tile was downloaded.
            }
            Ok(None) => {
                log::error!("IO thread is dead")
            }
        }
    }

    fn make_sure_is_downloaded(&mut self, tile_id: TileId) {
        if self
            .cache
            .try_get_or_insert(
                tile_id,
                || -> Result<Option<Texture>, TrySendError<TileId>> {
                    self.request_tx.try_send(tile_id)?;
                    log::trace!("Requested tile: {:?}", tile_id);
                    Ok(None)
                },
            )
            .is_err()
        {
            log::debug!("Request queue is full.");
        }
    }

    /// Get at tile, or interpolate it from lower zoom levels.
    fn get_or_interpolate(&mut self, tile_id: TileId) -> Option<TextureWithUv> {
        let mut zoom_candidate = tile_id.zoom;

        loop {
            let (zoomed_tile_id, uv) = interpolate_higher_zoom(tile_id, zoom_candidate);

            if let Some(Some(texture)) = self.cache.get(&zoomed_tile_id) {
                break Some(TextureWithUv {
                    texture: texture.clone(),
                    uv,
                });
            }

            // Keep zooming out until we find a donor or there is no more zoom levels.
            zoom_candidate = zoom_candidate.checked_sub(1)?;
        }
    }
}

/// Take a piece of a tile with higher zoom level and use it as a tile with lower zoom level.
fn interpolate_higher_zoom(tile_id: TileId, available_zoom: u8) -> (TileId, Rect) {
    assert!(tile_id.zoom >= available_zoom);

    let dzoom = 2u32.pow((tile_id.zoom - available_zoom) as u32);

    let x = (tile_id.x / dzoom, tile_id.x % dzoom);
    let y = (tile_id.y / dzoom, tile_id.y % dzoom);

    let zoomed_tile_id = TileId {
        x: x.0,
        y: y.0,
        zoom: available_zoom,
    };

    let z = (dzoom as f32).recip();

    let uv = Rect::from_min_max(
        pos2(x.1 as f32 * z, y.1 as f32 * z),
        pos2(x.1 as f32 * z + z, y.1 as f32 * z + z),
    );

    (zoomed_tile_id, uv)
}

impl Tiles for HttpTiles {
    /// Attribution of the source this tile cache pulls images from. Typically,
    /// this should be displayed somewhere on the top of the map widget.
    fn attribution(&self) -> Attribution {
        self.attribution.clone()
    }

    /// Return a tile if already in cache, schedule a download otherwise.
    fn at(&mut self, tile_id: TileId) -> Option<TextureWithUv> {
        self.put_single_downloaded_tile_in_cache();

        self.make_sure_is_downloaded(if tile_id.zoom > self.max_zoom {
            interpolate_higher_zoom(tile_id, self.max_zoom).0
        } else {
            tile_id
        });

        self.get_or_interpolate(tile_id)
    }

    fn tile_size(&self) -> u32 {
        self.tile_size
    }
}

/// Coordinates of the OSM-like tile.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct TileId {
    /// X number of the tile.
    pub x: u32,

    /// Y number of the tile.
    pub y: u32,

    /// Zoom level, where 0 means no zoom.
    /// See: <https://wiki.openstreetmap.org/wiki/Zoom_levels>
    pub zoom: u8,
}

impl TileId {
    /// Tile position (in pixels) on the "World bitmap".
    pub fn project(&self, tile_size: f64) -> Pixel {
        Pixel::new(self.x as f64 * tile_size, self.y as f64 * tile_size)
    }

    pub fn east(&self) -> Option<TileId> {
        Some(TileId {
            x: self.x + 1,
            y: self.y,
            zoom: self.zoom,
        })
    }

    pub fn west(&self) -> Option<TileId> {
        Some(TileId {
            x: self.x.checked_sub(1)?,
            y: self.y,
            zoom: self.zoom,
        })
    }

    pub fn north(&self) -> Option<TileId> {
        Some(TileId {
            x: self.x,
            y: self.y.checked_sub(1)?,
            zoom: self.zoom,
        })
    }

    pub fn south(&self) -> Option<TileId> {
        Some(TileId {
            x: self.x,
            y: self.y + 1,
            zoom: self.zoom,
        })
    }
}

/// Use simple [flood fill algorithm](https://en.wikipedia.org/wiki/Flood_fill) to draw tiles on the map.
pub(crate) fn flood_fill_tiles(
    viewport: Rect,
    tile_id: TileId,
    map_center_projected_position: Pixel,
    zoom: f64,
    tiles: &mut dyn Tiles,
    meshes: &mut HashMap<TileId, Option<Mesh>>,
) {
    // We need to make up the difference between integer and floating point zoom levels.
    let corrected_tile_size = tiles.tile_size() as f64 * 2f64.powf(zoom - zoom.round());
    let tile_projected = tile_id.project(corrected_tile_size);
    let tile_screen_position =
        viewport.center() + (tile_projected - map_center_projected_position).into();

    if viewport.intersects(rect(tile_screen_position, corrected_tile_size)) {
        if let Entry::Vacant(entry) = meshes.entry(tile_id) {
            // It's still OK to insert an empty one, as we need to mark the spot for the filling algorithm.
            let tile = tiles.at(tile_id).map(|tile| {
                tile.texture
                    .mesh_with_uv(tile_screen_position, corrected_tile_size, tile.uv)
            });

            entry.insert(tile);

            for next_tile_id in [
                tile_id.north(),
                tile_id.east(),
                tile_id.south(),
                tile_id.west(),
            ]
            .iter()
            .flatten()
            {
                flood_fill_tiles(
                    viewport,
                    *next_tile_id,
                    map_center_projected_position,
                    zoom,
                    tiles,
                    meshes,
                );
            }
        }
    }
}
