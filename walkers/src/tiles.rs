use std::collections::hash_map::Entry;
use std::collections::HashMap;

use egui::{pos2, Color32, Context, Mesh, Rect, Vec2};
use egui::{ColorImage, TextureHandle};
use image::ImageError;
use resvg::usvg::{Options, Transform};
use thiserror::Error;

use crate::mercator::{project, tile_id, total_tiles};
use crate::position::{Pixels, PixelsExt};
use crate::sources::Attribution;
use crate::zoom::Zoom;
use crate::Position;

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
pub struct Texture(TextureHandle);

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    ImageError(ImageError),
    #[error(transparent)]
    SvgError(#[from] resvg::usvg::Error),
    #[error(transparent)]
    MvtError(#[from] mvt_reader::error::ParserError),
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

    pub fn from_svg(data: &[u8], ctx: &Context) -> Result<Self, Error> {
        let tree = resvg::usvg::Tree::from_data(data, &Options::default())?;
        let mut pixmap = resvg::tiny_skia::Pixmap::new(256, 256).unwrap();
        resvg::render(&tree, Transform::default(), &mut pixmap.as_mut());
        let image = ColorImage::from_rgba_premultiplied(
            [pixmap.width() as usize, pixmap.height() as usize],
            pixmap.data(),
        );
        Ok(Self::from_color_image(image, ctx))
    }

    pub fn from_mvt(data: &[u8], ctx: &Context) -> Result<Self, Error> {
        let tile = mvt_reader::Reader::new(data.to_vec())?;
        let mut pixmap = resvg::tiny_skia::Pixmap::new(256, 256).unwrap();
        //for layer in tile.layers {
        //    for feature in layer.features {
        //        if let Some(geom) = feature.geometry() {
        //            let path = resvg::tiny_skia::PathBuilder::from_mvt_geom(&geom);
        //            let mut paint = resvg::tiny_skia::Paint::default();
        //            paint.set_color(resvg::tiny_skia::Color::from_rgba8(0, 0, 0, 255));
        //            pixmap.fill_path(
        //                &path,
        //                &paint,
        //                resvg::tiny_skia::FillRule::Winding,
        //                resvg::tiny_skia::Transform::identity(),
        //                None,
        //            );
        //        }
        //    }
        //}
        //let image = ColorImage::from_rgba_premultiplied(
        //    [pixmap.width() as usize, pixmap.height() as usize],
        //    pixmap.data(),
        //);
        //Ok(Self::from_color_image(image, ctx))
        todo!();
    }

    /// Load the texture from egui's [`ColorImage`].
    pub fn from_color_image(color_image: ColorImage, ctx: &Context) -> Self {
        Self(ctx.load_texture("image", color_image, Default::default()))
    }

    pub(crate) fn size(&self) -> Vec2 {
        self.0.size_vec2()
    }

    pub(crate) fn mesh_with_uv(
        &self,
        screen_position: Vec2,
        tile_size: f64,
        uv: Rect,
        transparency: f32,
    ) -> Mesh {
        self.mesh_with_rect_and_uv(rect(screen_position, tile_size), uv, transparency)
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

    pub(crate) fn mesh_with_rect_and_uv(&self, rect: Rect, uv: Rect, transparency: f32) -> Mesh {
        let mut mesh = Mesh::with_texture(self.0.id());
        mesh.add_rect_with_uv(rect, uv, Color32::WHITE.gamma_multiply(transparency));
        mesh
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

pub(crate) fn draw_tiles(
    painter: &egui::Painter,
    map_center: Position,
    zoom: Zoom,
    tiles: &mut dyn Tiles,
    transparency: f32,
) {
    let mut meshes = Default::default();
    flood_fill_tiles(
        painter.clip_rect(),
        tile_id(map_center, zoom.round(), tiles.tile_size()),
        project(map_center, zoom.into()),
        zoom.into(),
        tiles,
        transparency,
        &mut meshes,
    );

    for shape in meshes.drain().filter_map(|(_, mesh)| mesh) {
        painter.add(shape);
    }
}

/// Use simple [flood fill algorithm](https://en.wikipedia.org/wiki/Flood_fill) to draw tiles on the map.
fn flood_fill_tiles(
    viewport: Rect,
    tile_id: TileId,
    map_center_projected_position: Pixels,
    zoom: f64,
    tiles: &mut dyn Tiles,
    transparency: f32,
    meshes: &mut HashMap<TileId, Option<Mesh>>,
) {
    // We need to make up the difference between integer and floating point zoom levels.
    let corrected_tile_size = tiles.tile_size() as f64 * 2f64.powf(zoom - zoom.round());
    let tile_projected = tile_id.project(corrected_tile_size);
    let tile_screen_position =
        viewport.center().to_vec2() + (tile_projected - map_center_projected_position).to_vec2();

    if viewport.intersects(rect(tile_screen_position, corrected_tile_size)) {
        if let Entry::Vacant(entry) = meshes.entry(tile_id) {
            // It's still OK to insert an empty one, as we need to mark the spot for the filling algorithm.
            let tile = tiles.at(tile_id).map(|tile| {
                tile.texture.mesh_with_uv(
                    tile_screen_position,
                    corrected_tile_size,
                    tile.uv,
                    transparency,
                )
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
                    transparency,
                    meshes,
                );
            }
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

#[cfg(test)]
mod tests {
    use super::*;

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
