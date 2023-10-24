//! Extra functionalities that can be used with the map.

use egui::{vec2, Align2, Color32, FontId, Stroke};

use crate::{Plugin, Position};

#[derive(Default)]
pub struct Options {
    pub symbol_font: FontId,
    pub symbol_color: Color32,
    pub symbol_fill_color: Color32,
    pub symbol_stroke: Stroke,
}

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
    options: Options,
}

impl Places {
    pub fn new(places: Vec<Place>) -> Self {
        Self {
            places,
            options: Options::default(),
        }
    }

    pub fn with_options(places: Vec<Place>, options: Options) -> Self {
        Self { places, options }
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

            let label =
                painter.layout_no_wrap(place.label.to_owned(), FontId::default(), Color32::WHITE);

            // Offset of the label, relative to the circle.
            let offset = vec2(8., 8.);

            let style = painter.ctx().style();

            painter.rect_filled(
                label
                    .rect
                    .translate(screen_position)
                    .translate(offset)
                    .expand(5.),
                10.,
                semi_transparent(style.visuals.extreme_bg_color),
            );

            painter.galley_with_color(
                (screen_position + offset).to_pos2(),
                label,
                style.visuals.text_color(),
            );

            painter.circle(
                screen_position.to_pos2(),
                10.,
                self.options.symbol_fill_color,
                self.options.symbol_stroke,
            );

            painter.text(
                screen_position.to_pos2(),
                Align2::CENTER_CENTER,
                place.symbol.to_string(),
                self.options.symbol_font.clone(),
                self.options.symbol_color,
            );
        }
    }
}
