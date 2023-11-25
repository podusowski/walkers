use crate::tiles::Texture;
use crate::{Plugin, Position};
use egui::epaint::emath::Rot2;
use egui::{Painter, Rect, Response, Vec2};

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
    fn draw(&self, _response: &Response, painter: Painter, projector: &crate::Projector) {
        for image in &self.images {
            let rect = Rect::from_center_size(
                projector.project(image.position).to_pos2(),
                image.texture.size() * image.scale,
            );

            if painter.clip_rect().intersects(rect) {
                let mut mesh = image.texture.mesh_with_rect(rect);
                mesh.rotate(image.angle, rect.center());
                painter.add(mesh);
            }
        }
    }
}
