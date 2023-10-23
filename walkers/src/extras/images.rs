use crate::{Plugin, Position};
use egui::TextureId;
use egui::{pos2, Color32, Rect};

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
            let map_rect = painter.clip_rect();
            let rect = map_rect.translate(screen_position);

            let skip = (rect.max.x < map_rect.min.x)
                | (rect.max.y < map_rect.min.y)
                | (rect.min.x > map_rect.max.x)
                | (rect.min.y > map_rect.max.y);

            if skip {
                continue;
            }

            painter.image(
                image.texture,
                rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }
}
