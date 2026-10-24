#[cfg(feature = "mvt")]
use crate::mvt;
#[cfg(feature = "mvt")]
use crate::render;

use egui::{Color32, Context, Mesh, Rect, Vec2, pos2};
use egui::{ColorImage, TextureHandle};
use image::{ImageError, ImageReader};
use std::collections::HashSet;
use thiserror::Error;

use crate::io::TileFactory;
use crate::mercator::{TILE_SIZE, tile_id_at, total_tiles};
use crate::position::{Pixels, PixelsExt};
use crate::sources::Attribution;
use crate::style::Style;
use crate::zoom::Zoom;
use crate::{Canvas, Projector, Stats, ZOrder};

#[derive(Error, Debug)]
pub enum TileError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Image(#[from] ImageError),

    #[cfg(feature = "mvt")]
    #[error(transparent)]
    Mvt(#[from] mvt::Error),

    #[error("Tile data is empty.")]
    Empty,

    #[error("Unrecognized image format.")]
    UnrecognizedFormat,
}

/// Identifies the tile in the tile grid.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
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
    pub fn project(&self, tile_size: f64) -> Pixels {
        Pixels::new(self.x as f64 * tile_size, self.y as f64 * tile_size)
    }

    pub fn east(&self) -> Option<TileId> {
        (self.x < total_tiles(self.zoom) - 1).then_some(TileId {
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
        (self.y < total_tiles(self.zoom) - 1).then_some(TileId {
            x: self.x,
            y: self.y + 1,
            zoom: self.zoom,
        })
    }

    pub(crate) fn valid(&self) -> bool {
        self.x < total_tiles(self.zoom) && self.y < total_tiles(self.zoom)
    }
}

/// Source of tiles to be put together to render the map.
pub trait Tiles {
    fn at(&mut self, tile_id: TileId) -> Option<TilePiece>;
    fn attribution(&self) -> Attribution;

    /// Size of each tile, in pixels. Walkers works with 256px tiles internally, so this
    /// should be 256 multiplied or divided by a power of two, for example 128, 256 or 512.
    fn tile_size(&self) -> u32;

    /// Tiles being loaded right now.
    fn stats(&self) -> Stats {
        Stats::default()
    }
}

#[derive(Clone)]
pub enum Tile {
    Raster(TextureHandle),
    #[cfg(feature = "mvt")]
    Vector {
        /// What the tile decoded into. Held behind an `Arc` so that a mesh inside it stays at
        /// one address while the GPU holds buffers for it.
        drawables: std::sync::Arc<Vec<crate::render::drawable::Drawable>>,

        texts: Vec<crate::text::Text>,
    },
}

impl Tile {
    /// Create a tile from raw image data. The data can be either raster image (PNG, JPEG, etc.)
    /// or vector tile (MVT) if the `mvt` feature is enabled.
    pub fn new(
        image: &[u8],
        style: &Style,
        zoom: u8,
        tile_size: u32,
        ctx: &Context,
    ) -> Result<Self, TileError> {
        #[cfg(not(feature = "mvt"))]
        let _ = (style, zoom, tile_size);

        if image.is_empty() {
            return Err(TileError::Empty);
        }

        let reader = ImageReader::new(std::io::Cursor::new(image)).with_guessed_format()?;
        if reader.format().is_some() {
            log::debug!("Decoding tile as raster image.");
            let image = reader.decode()?.to_rgba8();
            let pixels = image.as_flat_samples();
            let image = ColorImage::from_rgba_unmultiplied(
                [image.width() as _, image.height() as _],
                pixels.as_slice(),
            );

            Ok(Self::from_color_image(image, ctx))
        } else {
            #[cfg(feature = "mvt")]
            {
                log::debug!("Trying to decode tile as MVT vector tile.");
                Ok(Self::from_mvt(image, style, zoom, tile_size)?)
            }
            #[cfg(not(feature = "mvt"))]
            {
                Err(TileError::UnrecognizedFormat)
            }
        }
    }

    #[cfg(feature = "mvt")]
    pub fn from_mvt(
        data: &[u8],
        style: &Style,
        zoom: u8,
        tile_size: u32,
    ) -> Result<Self, TileError> {
        let (drawables, texts) = mvt::render(data, style, zoom, tile_size)?;

        Ok(Self::Vector {
            drawables: std::sync::Arc::new(drawables),
            texts,
        })
    }

    /// Load the texture from egui's [`ColorImage`].
    fn from_color_image(color_image: ColorImage, ctx: &Context) -> Self {
        Self::Raster(ctx.load_texture("image", color_image, Default::default()))
    }

    /// Draw the tile on the given `rect`. The `uv` parameter defines which part of the tile
    /// should be drawn on the `rect`. `zoom` is the map's.
    #[allow(clippy::too_many_arguments)]
    fn draw(
        &self,
        canvas: &mut Canvas,
        rect: Rect,
        uv: Rect,
        transparency: f32,
        tile_size: u32,
        zoom: f64,
        z_order: ZOrder,
    ) {
        #[cfg(not(feature = "mvt"))]
        let _ = (tile_size, zoom, z_order);

        match self {
            Tile::Raster(texture_handle) => {
                let mut mesh = Mesh::with_texture(texture_handle.id());
                mesh.add_rect_with_uv(rect, uv, Color32::WHITE.gamma_multiply(transparency));
                canvas.add_shape(egui::Shape::mesh(mesh));
            }
            #[cfg(feature = "mvt")]
            Tile::Vector {
                drawables,
                texts: from_tile,
            } => {
                // Renderer needs to work on the full tile, before it was clipped with `uv`...
                let full_rect = full_rect_of_clipped_tile(rect, uv);
                let transform = mvt::transform_onto(full_rect, tile_size);

                // ...and then it can be clipped to the `rect`.
                canvas.add_drawables(
                    crate::Drawables::merged(drawables.to_owned())
                        .with_transform(transform)
                        .with_clip_rect(rect)
                        .with_transparency(transparency),
                );

                for text in render::transformed_texts(
                    from_tile,
                    transform,
                    transparency,
                    zoom - crate::mercator::zoom_offset(tile_size) as f64,
                ) {
                    canvas.add_text(z_order.combine(text.layer), text);
                }
            }
        }
    }
}

/// Clipped piece of a tile.
pub struct TilePiece {
    pub tile: Tile,
    pub uv: Rect,
}

impl TilePiece {
    pub fn new(tile: Tile, uv: Rect) -> Self {
        Self { tile, uv }
    }
}

pub(crate) fn draw_tiles(
    canvas: &mut Canvas,
    projector: &Projector,
    zoom: Zoom,
    tiles: &mut dyn Tiles,
    transparency: f32,
    z_order: ZOrder,
) {
    let mut meshes = Default::default();
    flood_fill_tiles(
        &Spread {
            clip_rect: projector.clip_rect(),
            map_center_projected_position: projector.map_center_projected_position(),
            zoom: zoom.into(),
            transparency,
            z_order,
        },
        tile_id_at(
            projector.map_center_projected_position(),
            zoom.into(),
            tiles.tile_size(),
        ),
        tiles,
        &mut meshes,
        canvas,
    );
}

/// What stays the same as the fill spreads from one tile to the next.
struct Spread {
    clip_rect: Rect,
    map_center_projected_position: Pixels,
    zoom: f64,
    transparency: f32,
    z_order: ZOrder,
}

/// Use simple [flood fill algorithm](https://en.wikipedia.org/wiki/Flood_fill) to draw tiles on the map.
fn flood_fill_tiles(
    spread: &Spread,
    tile_id: TileId,
    tiles: &mut dyn Tiles,
    meshes: &mut HashSet<TileId>,
    canvas: &mut Canvas,
) {
    let Spread {
        clip_rect,
        map_center_projected_position,
        zoom,
        transparency,
        z_order,
    } = *spread;
    // The tile's zoom level can differ from the map's: it is rounded to an integer, adjusted
    // for sources with tiles larger than 256px, and clamped at 0. Scale the tile so that it
    // covers the right amount of the map regardless.
    let corrected_tile_size = TILE_SIZE as f64 * 2f64.powf(zoom - tile_id.zoom as f64);
    let tile_projected = tile_id.project(corrected_tile_size);
    let tile_screen_position =
        clip_rect.center().to_vec2() + (tile_projected - map_center_projected_position).to_vec2();

    if clip_rect.intersects(rect(tile_screen_position, corrected_tile_size))
        && meshes.insert(tile_id)
    {
        if let Some(tile) = tiles.at(tile_id) {
            tile.tile.draw(
                canvas,
                rect(tile_screen_position, corrected_tile_size),
                tile.uv,
                transparency,
                tiles.tile_size(),
                zoom,
                z_order,
            )
        }

        for next_tile_id in [
            tile_id.north(),
            tile_id.east(),
            tile_id.south(),
            tile_id.west(),
        ]
        .iter()
        .flatten()
        {
            flood_fill_tiles(spread, *next_tile_id, tiles, meshes, canvas);
        }
    }
}

/// Take a piece of a tile with lower zoom level and use it as a required tile.
pub(crate) fn interpolate_from_lower_zoom(tile_id: TileId, available_zoom: u8) -> (TileId, Rect) {
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

#[cfg(any(feature = "mvt", test))]
/// Get the original rect which was clipped using the `uv`.
fn full_rect_of_clipped_tile(rect: Rect, uv: Rect) -> Rect {
    let uv_width = uv.max.x - uv.min.x;
    let uv_height = uv.max.y - uv.min.y;

    let full_width = rect.width() / uv_width;
    let full_height = rect.height() / uv_height;

    let full_min_x = rect.min.x - (full_width * uv.min.x);
    let full_min_y = rect.min.y - (full_height * uv.min.y);

    Rect::from_min_max(
        pos2(full_min_x, full_min_y),
        pos2(full_min_x + full_width, full_min_y + full_height),
    )
}

pub(crate) fn rect(screen_position: Vec2, tile_size: f64) -> Rect {
    Rect::from_min_size(screen_position.to_pos2(), Vec2::splat(tile_size as f32))
}

pub struct EguiTileFactory {
    egui_ctx: Context,
    style: Style,
    tile_size: u32,
}

impl EguiTileFactory {
    pub fn new(egui_ctx: Context, style: Style, tile_size: u32) -> Self {
        Self {
            egui_ctx,
            style,
            tile_size,
        }
    }
}

impl TileFactory for EguiTileFactory {
    fn create_tile(&self, data: &bytes::Bytes, zoom: u8) -> Result<Tile, TileError> {
        Tile::new(data, &self.style, zoom, self.tile_size, &self.egui_ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lon_lat;

    /// Records which tiles were asked for, without ever returning one.
    struct RecordingTiles {
        tile_size: u32,
        requested: Vec<TileId>,
    }

    impl RecordingTiles {
        fn new(tile_size: u32) -> Self {
            Self {
                tile_size,
                requested: Vec::new(),
            }
        }
    }

    impl Tiles for RecordingTiles {
        fn at(&mut self, tile_id: TileId) -> Option<TilePiece> {
            self.requested.push(tile_id);
            None
        }

        fn attribution(&self) -> Attribution {
            Attribution {
                text: "",
                url: "",
                logo_light: None,
                logo_dark: None,
            }
        }

        fn tile_size(&self) -> u32 {
            self.tile_size
        }
    }

    /// Projector for a square viewport of `size`, centered somewhere in Warsaw.
    fn projector_at(size: f32, zoom: f64) -> Projector<'static> {
        let mut memory = crate::MapMemory::default();
        #[allow(clippy::unwrap_used)]
        memory.set_zoom(zoom).unwrap();
        Projector::new(
            Rect::from_min_size(pos2(0., 0.), Vec2::splat(size)),
            &crate::MercatorProjection,
            &memory,
            lon_lat(21.00027, 52.26470),
        )
    }

    /// Run [`draw_tiles`] on a viewport of `TILE_SIZE` squared, and report which tiles the
    /// source was asked for.
    fn requested_tiles(tile_size: u32, zoom: f64) -> Vec<TileId> {
        let mut tiles = RecordingTiles::new(tile_size);

        #[allow(clippy::unwrap_used)]
        draw_tiles(
            &mut Canvas::new(&Context::default()),
            &projector_at(TILE_SIZE as f32, zoom),
            Zoom::try_from(zoom).unwrap(),
            &mut tiles,
            1.0,
            ZOrder::default(),
        );

        tiles.requested
    }

    /// Sources with tiles smaller than 256px are just as valid as the larger ones, and the
    /// map should not go blank when one is used.
    /// See: <https://github.com/podusowski/walkers/issues/551>
    #[test]
    fn smaller_tiles_are_requested_from_a_higher_zoom_level() {
        let zoom = 16.;

        // A 256px source covers the viewport with tiles from the map's own zoom level.
        let with_256 = requested_tiles(256, zoom);
        assert!(!with_256.is_empty());
        assert!(with_256.iter().all(|tile_id| tile_id.zoom == 16));

        // 128px tiles are half the size, so they have to come from one zoom level deeper to
        // show the same area at the same scale, and more of them are needed to fill the
        // viewport.
        let with_128 = requested_tiles(128, zoom);
        assert!(with_128.iter().all(|tile_id| tile_id.zoom == 17));
        assert!(with_128.len() > with_256.len());

        // Same story one step further down.
        let with_64 = requested_tiles(64, zoom);
        assert!(with_64.iter().all(|tile_id| tile_id.zoom == 18));
        assert!(with_64.len() > with_128.len());
    }

    #[test]
    fn test_full_rect_of_clipped_tile() {
        let rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(50.0, 50.0));
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(0.5, 0.5));

        let full_rect = full_rect_of_clipped_tile(rect, uv);

        assert_eq!(full_rect.min, pos2(0.0, 0.0));
        assert_eq!(full_rect.max, pos2(100.0, 100.0));
    }

    #[test]
    fn tile_id_cannot_go_beyond_limits() {
        // There is only one tile at zoom 0.
        let tile_id = TileId {
            x: 0,
            y: 0,
            zoom: 0,
        };

        assert_eq!(tile_id.west(), None);
        assert_eq!(tile_id.north(), None);
        assert_eq!(tile_id.south(), None);
        assert_eq!(tile_id.east(), None);

        // There are 2 tiles at zoom 1.
        let tile_id = TileId {
            x: 0,
            y: 0,
            zoom: 1,
        };

        assert_eq!(tile_id.west(), None);
        assert_eq!(tile_id.north(), None);

        assert_eq!(
            tile_id.south(),
            Some(TileId {
                x: 0,
                y: 1,
                zoom: 1
            })
        );

        assert_eq!(
            tile_id.east(),
            Some(TileId {
                x: 1,
                y: 0,
                zoom: 1
            })
        );
    }

    /// A source whose every tile carries the same label at both its left and right edge, the
    /// way vector tiles repeat features which fall near a boundary. Positions are in the
    /// pixels of a tile, which is what a rendered tile holds.
    #[cfg(feature = "mvt")]
    struct LabelAtBothEdges;

    #[cfg(feature = "mvt")]
    impl Tiles for LabelAtBothEdges {
        fn at(&mut self, _tile_id: TileId) -> Option<TilePiece> {
            let label = |x: f32| {
                crate::text::Text::new(
                    pos2(x, TILE_SIZE as f32 / 2.),
                    "Szczepankowice".to_string(),
                    12.,
                    Color32::BLACK,
                    0.,
                )
            };

            Some(TilePiece::new(
                Tile::Vector {
                    drawables: std::sync::Arc::new(Vec::new()),
                    texts: vec![label(0.), label(TILE_SIZE as f32)],
                },
                Rect::from_min_max(pos2(0., 0.), pos2(1., 1.)),
            ))
        }

        fn attribution(&self) -> Attribution {
            Attribution {
                text: "",
                url: "",
                logo_light: None,
                logo_dark: None,
            }
        }

        fn tile_size(&self) -> u32 {
            TILE_SIZE
        }
    }

    /// The same label arriving from two neighbouring tiles should be drawn once. Before labels
    /// were gathered across tiles, each tile placed its own copy.
    #[cfg(feature = "mvt")]
    #[test]
    fn a_label_shared_by_two_tiles_is_placed_once() {
        let ctx = Context::default();
        // Fonts only exist once a pass has been run.
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();

        let mut canvas = Canvas::new(&ctx);
        #[allow(clippy::unwrap_used)]
        draw_tiles(
            &mut canvas,
            &projector_at(TILE_SIZE as f32 * 2., 16.),
            Zoom::try_from(16.).unwrap(),
            &mut LabelAtBothEdges,
            1.0,
            ZOrder::default(),
        );

        let gathered = canvas.texts.len();
        let positions: std::collections::HashSet<_> = canvas
            .texts
            .iter()
            .map(|(_, text)| (text.position.x as i32, text.position.y as i32))
            .collect();

        // Neighbouring tiles put a label on the very same spot.
        assert!(
            gathered > positions.len(),
            "{gathered} labels landed on {} distinct spots, so none were shared",
            positions.len()
        );

        let texts = canvas.texts.into_iter().map(|(_, text)| text).collect();
        let placed = crate::text::place_texts(texts, &ctx)
            .into_iter()
            .filter(|shape| !matches!(shape, egui::Shape::Noop))
            .count();

        assert_eq!(
            placed,
            positions.len(),
            "expected one label per spot, got {placed} for {} spots",
            positions.len()
        );
    }
}
