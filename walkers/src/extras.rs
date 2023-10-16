use egui::{Align2, Color32, Shape, Stroke, Vec2};

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
            let ctx = painter.ctx();

            painter.circle(
                screen_position.to_pos2(),
                10.,
                Color32::WHITE,
                Stroke::new(3., Color32::BLACK),
            );

            let background = |text: &Shape| {
                Shape::rect_filled(
                    text.visual_bounding_rect().expand(5.),
                    5.,
                    ctx.style().visuals.extreme_bg_color,
                )
            };

            let text = ctx.fonts(|fonts| {
                Shape::text(
                    fonts,
                    screen_position.to_pos2() + Vec2::new(5., 5.),
                    Align2::LEFT_TOP,
                    &place.label,
                    Default::default(),
                    ctx.style().visuals.text_color(),
                )
            });
            painter.add(background(&text));
            painter.add(text);
        }
    }
}
