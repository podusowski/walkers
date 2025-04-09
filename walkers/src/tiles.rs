use egui::{pos2, Color32, Context, Mesh, Rect, Vec2};
use egui::{ColorImage, TextureHandle};
use image::ImageError;

use crate::mercator::TileId;
use crate::sources::Attribution;

// Source of tiles to be put together to render the map.
pub trait Tiles {
    fn at(&mut self, tile_id: TileId) -> Option<TextureWithUv>;
    fn attribution(&self) -> Attribution;
    fn tile_size(&self) -> u32;
}

pub(crate) fn rect(screen_position: Vec2, tile_size: f64) -> Rect {
    Rect::from_min_size(screen_position.to_pos2(), Vec2::splat(tile_size as f32))
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
