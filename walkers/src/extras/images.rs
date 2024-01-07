use crate::tiles::Texture;
use crate::{Plugin, Position};
use egui::epaint::emath::Rot2;
use egui::{Painter, Rect, Response, Vec2};

/// An image to be drawn on the map.
pub struct Image {
    /// Geographical position.
    position: Position,

    scale: Vec2,
    angle: Rot2,
    texture: Texture,
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

    pub fn draw(&self, _response: &Response, painter: Painter, projector: &crate::Projector) {
        let rect = Rect::from_center_size(
            projector.project(self.position).to_pos2(),
            self.texture.size() * self.scale,
        );

        if painter.clip_rect().intersects(rect) {
            let mut mesh = self.texture.mesh_with_rect(rect);
            mesh.rotate(self.angle, rect.center());
            painter.add(mesh);
        }
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
    fn draw(
        &self,
        response: &Response,
        _gesture_handled: bool,
        painter: Painter,
        projector: &crate::Projector,
    ) {
        for image in &self.images {
            image.draw(response, painter.clone(), projector);
        }
    }
}
