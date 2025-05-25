use egui::{emath::Rot2, Rect, Response, Ui, Vec2};

use crate::{Plugin, Position};

use super::Texture;

/// [`Plugin`] which draws list of places on the map.
pub struct Places<T>
where
    T: Place,
{
    places: Vec<T>,
}

impl<T> Places<T>
where
    T: Place,
{
    pub fn new(places: Vec<T>) -> Self {
        Self { places }
    }
}

impl<T> Plugin for Places<T>
where
    T: Place + 'static,
{
    fn run(self: Box<Self>, ui: &mut Ui, _response: &Response, projector: &crate::Projector) {
        for place in &self.places {
            place.draw(ui, projector);
        }
    }
}

pub trait Place {
    fn draw(&self, ui: &Ui, projector: &crate::Projector);
}

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
}

impl Place for Image {
    fn draw(&self, ui: &Ui, projector: &crate::Projector) {
        let painter = ui.painter();
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
