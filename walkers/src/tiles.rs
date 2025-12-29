use crate::mvt::OrientedRect;
#[cfg(feature = "mvt")]
use crate::mvt::{self, ShapeOrText, Text};

use egui::{Color32, Context, Mesh, Rect, Vec2, pos2};
use egui::{ColorImage, TextureHandle};
#[cfg(feature = "mvt")]
use egui::{FontId, Shape};
use image::{ImageError, ImageReader};
use std::collections::HashSet;
use thiserror::Error;

use crate::Position;
use crate::io::TileFactory;
use crate::mercator::{project, tile_id, total_tiles};
use crate::position::{Pixels, PixelsExt};
use crate::sources::Attribution;
use crate::style::Style;
use crate::zoom::Zoom;

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
    fn tile_size(&self) -> u32;
}

#[derive(Clone)]
pub enum Tile {
    Raster(TextureHandle),
    #[cfg(feature = "mvt")]
    Vector(Vec<ShapeOrText>),
}

impl Tile {
    /// Create a tile from raw image data. The data can be either raster image (PNG, JPEG, etc.)
    /// or vector tile (MVT) if the `mvt` feature is enabled.
    pub fn new(image: &[u8], style: &Style, zoom: u8, ctx: &Context) -> Result<Self, TileError> {
        #[cfg(not(feature = "mvt"))]
        let _ = style;

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
                Ok(Self::from_mvt(image, style, zoom)?)
            }
            #[cfg(not(feature = "mvt"))]
            {
                Err(TileError::UnrecognizedFormat)
            }
        }
    }

    #[cfg(feature = "mvt")]
    pub fn from_mvt(data: &[u8], style: &Style, zoom: u8) -> Result<Self, TileError> {
        Ok(Self::Vector(mvt::render(data, style, zoom)?))
    }

    /// Load the texture from egui's [`ColorImage`].
    fn from_color_image(color_image: ColorImage, ctx: &Context) -> Self {
        Self::Raster(ctx.load_texture("image", color_image, Default::default()))
    }

    /// Draw the tile on the given `rect`. The `uv` parameter defines which part of the tile
    /// should be drawn on the `rect`.
    fn draw(&self, painter: &egui::Painter, rect: Rect, uv: Rect, transparency: f32) {
        match self {
            Tile::Raster(texture_handle) => {
                let mut mesh = Mesh::with_texture(texture_handle.id());
                mesh.add_rect_with_uv(rect, uv, Color32::WHITE.gamma_multiply(transparency));
                painter.add(egui::Shape::mesh(mesh));
            }
            #[cfg(feature = "mvt")]
            Tile::Vector(shapes) => {
                // Renderer needs to work on the full tile, before it was clipped with `uv`...
                let full_rect = full_rect_of_clipped_tile(rect, uv);

                // ...and then it can be clipped to the `rect`.
                let painter = painter.with_clip_rect(rect);

                let mut occupied_text_areas = OccupiedAreas::new();

                // Need to collect it to avoid deadlock caused by `Painter::extend` and `fonts_mut`.
                let shapes: Vec<_> = mvt::transformed(shapes, full_rect)
                    .into_iter()
                    .map(|shape_or_text| match shape_or_text {
                        ShapeOrText::Shape(shape) => shape,
                        ShapeOrText::Text(text) => {
                            self.draw_text(text, painter.ctx(), &mut occupied_text_areas)
                        }
                    })
                    .collect();

                painter.extend(shapes);
            }
        }
    }

    #[cfg(feature = "mvt")]
    fn draw_text(
        &self,
        text: Text,
        ctx: &Context,
        occupied_text_areas: &mut OccupiedAreas,
    ) -> Shape {
        ctx.fonts_mut(|fonts| {
            use crate::mvt::OrientedRect;
            use egui::epaint::TextShape;

            let mut layout_job = egui::text::LayoutJob::default();

            layout_job.append(
                &text.text,
                0.0,
                egui::TextFormat {
                    font_id: FontId::proportional(text.font_size),
                    color: text.text_color,
                    background: text.background_color,
                    ..Default::default()
                },
            );

            let galley = fonts.layout_job(layout_job);
            let area = OrientedRect::new(&text, galley.size());
            let p0 = area.top_left();

            if occupied_text_areas.try_occupy(area) {
                TextShape::new(p0, galley, text.text_color)
                    .with_angle(text.angle)
                    .into()
            } else {
                Shape::Noop
            }
        })
    }
}

// Tracks areas occupied by texts to avoid overlapping them.
struct OccupiedAreas {
    areas: Vec<OrientedRect>,
}

impl OccupiedAreas {
    fn new() -> Self {
        Self { areas: Vec::new() }
    }

    fn try_occupy(&mut self, rect: OrientedRect) -> bool {
        if !self.areas.iter().any(|existing| existing.intersects(&rect)) {
            self.areas.push(rect);
            true
        } else {
            false
        }
    }
}

/// Clipped piece of a tile.
pub struct TilePiece {
    pub texture: Tile,
    pub uv: Rect,
}

impl TilePiece {
    pub fn new(texture: Tile, uv: Rect) -> Self {
        Self { texture, uv }
    }
}

pub(crate) fn draw_tiles(
    painter: &egui::Painter,
    map_center: Position,
    zoom: Zoom,
    tiles: &mut dyn Tiles,
    transparency: f32,
) {
    let mut meshes = Default::default();
    flood_fill_tiles(
        painter,
        tile_id(map_center, zoom.round(), tiles.tile_size()),
        project(map_center, zoom.into()),
        zoom.into(),
        tiles,
        transparency,
        &mut meshes,
    );
}

/// Use simple [flood fill algorithm](https://en.wikipedia.org/wiki/Flood_fill) to draw tiles on the map.
fn flood_fill_tiles(
    painter: &egui::Painter,
    tile_id: TileId,
    map_center_projected_position: Pixels,
    zoom: f64,
    tiles: &mut dyn Tiles,
    transparency: f32,
    meshes: &mut HashSet<TileId>,
) {
    // We need to make up the difference between integer and floating point zoom levels.
    let corrected_tile_size = tiles.tile_size() as f64 * 2f64.powf(zoom - zoom.round());
    let tile_projected = tile_id.project(corrected_tile_size);
    let tile_screen_position = painter.clip_rect().center().to_vec2()
        + (tile_projected - map_center_projected_position).to_vec2();

    if painter
        .clip_rect()
        .intersects(rect(tile_screen_position, corrected_tile_size))
        && meshes.insert(tile_id)
    {
        if let Some(tile) = tiles.at(tile_id) {
            tile.texture.draw(
                painter,
                rect(tile_screen_position, corrected_tile_size),
                tile.uv,
                transparency,
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
            flood_fill_tiles(
                painter,
                *next_tile_id,
                map_center_projected_position,
                zoom,
                tiles,
                transparency,
                meshes,
            );
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
}

impl EguiTileFactory {
    pub fn new(egui_ctx: Context, style: Style) -> Self {
        Self { egui_ctx, style }
    }
}

impl TileFactory for EguiTileFactory {
    fn create_tile(&self, data: &bytes::Bytes, zoom: u8) -> Result<Tile, TileError> {
        Tile::new(data, &self.style, zoom, &self.egui_ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
