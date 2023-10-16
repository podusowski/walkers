use egui::{Align2, Color32, Shape, Stroke, Vec2};

use crate::{Plugin, Position};

pub struct Places {
    places: Vec<Position>,
}

impl Places {
    pub fn new(places: Vec<Position>) -> Self {
        Self { places }
    }
}

impl Plugin for Places {
    fn draw(&self, painter: egui::Painter, projector: &crate::Projector) {
        for position in &self.places {
            let screen_position = projector.project(*position);
            let ctx = painter.ctx();

            {
                let x = Vec2::new(7., 0.);
                let y = Vec2::new(0., 7.);
                let stroke = Stroke::new(4., Color32::GRAY);

                painter.line_segment(
                    [screen_position.to_pos2(), (screen_position + x).to_pos2()],
                    stroke,
                );

                painter.line_segment(
                    [screen_position.to_pos2(), (screen_position + y).to_pos2()],
                    stroke,
                );

                painter.line_segment(
                    [
                        screen_position.to_pos2() + x + x + y,
                        (screen_position + x + x + y + y).to_pos2(),
                    ],
                    stroke,
                );

                painter.line_segment(
                    [
                        screen_position.to_pos2() + x + y + y,
                        (screen_position + x + x + y + y).to_pos2(),
                    ],
                    stroke,
                );
            }

            //painter.circle_stroke(
            //    screen_position.to_pos2(),
            //    5.,
            //    Stroke::new(3., Color32::DARK_BLUE),
            //);

            //let background = |text: &Shape| {
            //    Shape::rect_filled(
            //        text.visual_bounding_rect().expand(5.),
            //        5.,
            //        ctx.style().visuals.extreme_bg_color,
            //    )
            //};

            //let text = ctx.fonts(|fonts| {
            //    Shape::text(
            //        fonts,
            //        screen_position.to_pos2() + Vec2::new(10., 0.),
            //        Align2::LEFT_CENTER,
            //        "⬉ Here you can board the 106 line\nwhich goes to the airport.",
            //        Default::default(),
            //        ctx.style().visuals.text_color(),
            //    )
            //});
            //painter.add(background(&text));
            //painter.add(text);
        }
    }
}
