#[cfg(feature = "vector_tiles")]
use crate::mvt;
use std::collections::HashSet;

#[cfg(feature = "vector_tiles")]
use egui::Shape;
use egui::{Color32, Context, Mesh, Rect, Vec2, pos2};
use egui::{ColorImage, TextureHandle};
use image::ImageError;

use crate::Position;
use crate::mercator::{tile_id, total_tiles};
use crate::position::{Pixels, PixelsExt};
use crate::sources::Attribution;
use crate::zoom::Zoom;

/// Source of tiles to be put together to render the map.
pub trait Tiles {
    fn at(&mut self, tile_id: TileId) -> Option<TextureWithUv>;
    fn attribution(&self) -> Attribution;
    fn tile_size(&self) -> u32;
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

pub(crate) fn rect(screen_position: Vec2, tile_size: f64) -> Rect {
    Rect::from_min_size(screen_position.to_pos2(), Vec2::splat(tile_size as f32))
}

#[derive(Clone)]
pub enum Texture {
    Raster(TextureHandle),
    #[cfg(feature = "vector_tiles")]
    Vector(Vec<Shape>),
}

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
        Self::Raster(ctx.load_texture("image", color_image, Default::default()))
    }

    #[cfg(feature = "vector_tiles")]
    pub fn from_mvt(data: &[u8]) -> Result<Self, mvt::Error> {
        let reader = mvt_reader::Reader::new(data.to_vec())?;
        let shapes = mvt::render(&reader)?;
        Ok(Self::Vector(shapes))
    }

    /// Draw the tile on the given `rect`. The `uv` parameter defines which part of the tile
    /// should be drawn on the `rect`.
    fn draw(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        uv: Rect,
        transparency: f32,
        rotation: f32,
        center: egui::Pos2,
    ) {
        match self {
            Texture::Raster(texture_handle) => {
                let mut mesh = Mesh::with_texture(texture_handle.id());

                if rotation.abs() > 0.001 {
                    // Apply rotation to the mesh vertices
                    let cos = rotation.cos();
                    let sin = rotation.sin();
                    let center_vec = center.to_vec2();

                    // Get the four corners of the rect
                    let corners = [
                        rect.min,
                        pos2(rect.max.x, rect.min.y),
                        rect.max,
                        pos2(rect.min.x, rect.max.y),
                    ];

                    // Rotate each corner around the center
                    let rotated_corners: Vec<_> = corners
                        .iter()
                        .map(|&corner| {
                            let translated = corner.to_vec2() - center_vec;
                            let rotated = Vec2::new(
                                translated.x * cos - translated.y * sin,
                                translated.x * sin + translated.y * cos,
                            );
                            (rotated + center_vec).to_pos2()
                        })
                        .collect();

                    // UV coordinates for the corners
                    let uv_corners = [
                        pos2(uv.min.x, uv.min.y),
                        pos2(uv.max.x, uv.min.y),
                        pos2(uv.max.x, uv.max.y),
                        pos2(uv.min.x, uv.max.y),
                    ];

                    // Add the rotated quad to the mesh
                    let color = Color32::WHITE.gamma_multiply(transparency);
                    let idx = mesh.vertices.len() as u32;
                    for i in 0..4 {
                        mesh.vertices.push(egui::epaint::Vertex {
                            pos: rotated_corners[i],
                            uv: uv_corners[i],
                            color,
                        });
                    }
                    mesh.indices
                        .extend_from_slice(&[idx, idx + 1, idx + 2, idx, idx + 2, idx + 3]);
                } else {
                    mesh.add_rect_with_uv(rect, uv, Color32::WHITE.gamma_multiply(transparency));
                }

                painter.add(egui::Shape::mesh(mesh));
            }
            #[cfg(feature = "vector_tiles")]
            Texture::Vector(shapes) => {
                // Renderer needs to work on the full tile, before it was clipped with `uv`...
                let full_rect = full_rect_of_clipped_tile(rect, uv);

                // ...and then it can be clipped to the `rect`.
                let painter = painter.with_clip_rect(rect);

                painter.extend(mvt::transformed(shapes, full_rect));
            }
        }
    }
}

/// Texture with UV coordinates.
pub struct TextureWithUv {
    pub texture: Texture,
    pub uv: Rect,
}

impl TextureWithUv {
    pub fn new(texture: Texture, uv: Rect) -> Self {
        Self { texture, uv }
    }
}

pub(crate) struct TileDrawContext<'a> {
    pub painter: &'a egui::Painter,
    pub map_center_projected_position: Pixels,
    pub zoom: Zoom,
    pub tiles: &'a mut dyn Tiles,
    pub transparency: f32,
    pub rotation: f32,
}

pub(crate) fn draw_tiles(ctx: &mut TileDrawContext, map_center: Position) {
    let tile_size = ctx.tiles.tile_size();
    let mut meshes = HashSet::default();
    let initial_tile = tile_id(map_center, ctx.zoom.round(), tile_size);
    flood_fill_tiles(ctx, initial_tile, &mut meshes);
}

/// Use simple [flood fill algorithm](https://en.wikipedia.org/wiki/Flood_fill) to draw tiles on the map.
fn flood_fill_tiles(ctx: &mut TileDrawContext, tile_id: TileId, meshes: &mut HashSet<TileId>) {
    // We need to make up the difference between integer and floating point zoom levels.
    let zoom_f64: f64 = ctx.zoom.into();
    let corrected_tile_size = ctx.tiles.tile_size() as f64 * 2f64.powf(zoom_f64 - zoom_f64.round());
    let tile_projected = tile_id.project(corrected_tile_size);
    let tile_screen_position = ctx.painter.clip_rect().center().to_vec2()
        + (tile_projected - ctx.map_center_projected_position).to_vec2();

    // Check if the tile (when rotated) will intersect the viewport
    let tile_rect = rect(tile_screen_position, corrected_tile_size);
    let intersects = if ctx.rotation.abs() > 0.001 {
        // Calculate the bounding box of the rotated tile
        let center = ctx.painter.clip_rect().center();
        let cos = ctx.rotation.cos();
        let sin = ctx.rotation.sin();

        // Get corners of the tile
        let corners = [
            tile_rect.min,
            pos2(tile_rect.max.x, tile_rect.min.y),
            tile_rect.max,
            pos2(tile_rect.min.x, tile_rect.max.y),
        ];

        // Rotate corners and find bounding box
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;

        for corner in corners {
            let offset = corner - center;
            let rotated = Vec2::new(
                offset.x * cos - offset.y * sin,
                offset.x * sin + offset.y * cos,
            );
            let rotated_pos = center.to_vec2() + rotated;
            min_x = min_x.min(rotated_pos.x);
            min_y = min_y.min(rotated_pos.y);
            max_x = max_x.max(rotated_pos.x);
            max_y = max_y.max(rotated_pos.y);
        }

        let rotated_bounds = Rect::from_min_max(pos2(min_x, min_y), pos2(max_x, max_y));
        ctx.painter.clip_rect().intersects(rotated_bounds)
    } else {
        ctx.painter.clip_rect().intersects(tile_rect)
    };

    if intersects && meshes.insert(tile_id) {
        if let Some(tile) = ctx.tiles.at(tile_id) {
            tile.texture.draw(
                ctx.painter,
                rect(tile_screen_position, corrected_tile_size),
                tile.uv,
                ctx.transparency,
                ctx.rotation,
                ctx.painter.clip_rect().center(),
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
            flood_fill_tiles(ctx, *next_tile_id, meshes);
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

#[cfg(any(feature = "vector_tiles", test))]
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
