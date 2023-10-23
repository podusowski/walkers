use egui::{vec2, Align2, Color32, FontId, Stroke};

use crate::{Plugin, Position};

/// A place to be drawn on the map.
pub struct Place {
    /// Geographical position.
    pub position: Position,

    /// Text displayed next to the marker.
    pub label: String,

    /// Symbol drawn on the marker.
    pub symbol: char,
}

/// [`Plugin`] which draws given list of places on the map.
pub struct Places {
    places: Vec<Place>,
}

impl Places {
    pub fn new(places: Vec<Place>) -> Self {
        Self { places }
    }
}

fn semi_transparent(mut color: Color32) -> Color32 {
    color[3] = 200;
    color
}

impl Plugin for Places {
    fn draw(&self, painter: egui::Painter, projector: &crate::Projector) {
        for place in &self.places {
            let screen_position = projector.project(place.position);

            let galley =
                painter.layout_no_wrap(place.label.to_owned(), FontId::default(), Color32::WHITE);

            // Offset of the label, relative to the circle.
            let offset = vec2(8., 8.);

            let style = painter.ctx().style();

            painter.rect_filled(
                galley
                    .rect
                    .translate(screen_position)
                    .translate(offset)
                    .expand(5.),
                10.,
                semi_transparent(style.visuals.extreme_bg_color),
            );

            painter.galley_with_color(
                (screen_position + offset).to_pos2(),
                galley,
                style.visuals.text_color(),
            );

            painter.circle(
                screen_position.to_pos2(),
                12.,
                Color32::WHITE,
                Stroke::new(3., style.visuals.extreme_bg_color),
            );

            painter.text(
                screen_position.to_pos2(),
                Align2::CENTER_CENTER,
                place.symbol.to_string(),
                FontId::default(),
                Color32::BLACK,
            );
        }
    }
}
