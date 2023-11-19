use crate::tiles::Texture;
use crate::{Plugin, Position};
use egui::epaint::emath::Rot2;
use egui::Vec2;

/// An image to be drawn on the map.
pub struct Image {
    /// Geographical position.
    pub position: Position,

    scale: Vec2,
    angle: Rot2,
    pub texture: Texture,
}

impl Image {
    pub fn new(texture: Texture, position: Position) -> Self {
        Self {
            position,
            scale: Vec2::splat(1.0),
            angle: Rot2::from_angle(0.0),
            texture,
        }
    }

    /// Scale the image.
    pub fn scale(&mut self, x: f32, y: f32) {
        self.scale.x = x;
        self.scale.y = y;
    }

    /// Set the image's angle in radians.
    pub fn angle(&mut self, angle: f32) {
        self.angle = Rot2::from_angle(angle);
    }
}

/// [`Plugin`] which draws given list of images on the map.
pub struct Images {
    images: Vec<Image>,
}

impl Images {
    pub fn new(images: Vec<Image>) -> Self {
        Self { images }
    }
}

impl Plugin for Images {
    fn draw(&self, painter: egui::Painter, projector: &crate::Projector) {
        for image in &self.images {
            let screen_position = projector.project(image.position);
            let viewport = painter.clip_rect();
            let texture = &image.texture;

            let size = texture.size();
            let w = size.x as f32 * image.scale.x;
            let h = size.y as f32 * image.scale.y;
            let mut rect = viewport.translate(screen_position);

            rect.min.x -= w / 2.0;
            rect.min.y -= h / 2.0;

            rect.max.x = rect.min.x + w;
            rect.max.y = rect.min.y + h;

            if viewport.intersects(rect) {
                let mut mesh = image.texture.mesh_with_rect(rect);
                mesh.rotate(image.angle, rect.center());
                painter.add(mesh);
            }
        }
    }
}
