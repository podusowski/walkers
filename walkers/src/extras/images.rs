use crate::{Plugin, Position};
use egui::TextureId;
use egui::{pos2, Color32, Rect, Stroke};

/// A image to be drawn on the map.
pub struct Image {
    /// Geographical position.
    pub position: Position,

    /// Texture id of image.
    pub texture: TextureId,
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
            let style = painter.ctx().style();

            painter.circle(
                screen_position.to_pos2(),
                12.,
                Color32::WHITE,
                Stroke::new(3., style.visuals.extreme_bg_color),
            );

            let rect = painter.clip_rect().translate(screen_position);

            painter.image(
                image.texture,
                rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }
}
