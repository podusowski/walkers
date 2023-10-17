use egui::{vec2, Color32, FontId, Stroke};

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

            // Offset of the label, relative to the circle.
            let offset = vec2(5., 5.);

            let style = painter.ctx().style();

            painter.rect_filled(
                galley
                    .rect
                    .translate(screen_position)
                    .translate(offset)
                    .expand(5.),
                6.,
                style.visuals.extreme_bg_color,
            );

            painter.galley_with_color(
                (screen_position + offset).to_pos2(),
                galley,
                style.visuals.text_color(),
            );

            painter.circle(
                screen_position.to_pos2(),
                6.,
                Color32::WHITE,
                Stroke::new(3., style.visuals.extreme_bg_color),
            );
        }
    }
}
