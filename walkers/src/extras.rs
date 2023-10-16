use egui::{vec2, Align2, Color32, FontId, Shape, Stroke, Vec2};

use crate::{Plugin, Position};

pub struct Place {
    pub position: Position,
    pub label: String,
}

pub struct Places {
    places: Vec<Place>,
}

impl Places {
    pub fn new(places: Vec<Place>) -> Self {
        Self { places }
    }
}

impl Plugin for Places {
    fn draw(&self, painter: egui::Painter, projector: &crate::Projector) {
        for place in &self.places {
            let screen_position = projector.project(place.position);

            let galley =
                painter.layout_no_wrap(place.label.to_owned(), FontId::default(), Color32::WHITE);

            let offset = vec2(5., 5.);

            painter.rect_filled(
                galley
                    .rect
                    .translate(screen_position)
                    .translate(offset)
                    .expand(5.),
                6.,
                Color32::BLACK,
            );

            painter.galley((screen_position + offset).to_pos2(), galley);

            painter.circle(
                screen_position.to_pos2(),
                6.,
                Color32::WHITE,
                Stroke::new(3., Color32::BLACK),
            );
        }
    }
}
