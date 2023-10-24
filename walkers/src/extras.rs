//! Extra functionalities that can be used with the map.

use egui::{vec2, Align2, Color32, FontId, Stroke};

use crate::{Plugin, Position};

/// Visual style of the place.
#[derive(Clone)]
pub struct Style {
    pub label_font: FontId,
    pub label_text_color: Color32,
    pub label_background: Color32,
    pub symbol_font: FontId,
    pub symbol_color: Color32,
    pub symbol_fill_color: Color32,
    pub symbol_stroke: Stroke,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            label_font: FontId::proportional(12.),
            label_text_color: Color32::from_gray(200),
            label_background: Color32::BLACK.gamma_multiply(0.8),
            symbol_font: FontId::proportional(14.),
            symbol_color: Color32::BLACK.gamma_multiply(0.8),
            symbol_fill_color: Color32::WHITE.gamma_multiply(0.8),
            symbol_stroke: Stroke::new(2., Color32::BLACK.gamma_multiply(0.8)),
        }
    }
}

/// A place to be drawn on the map.
pub struct Place {
    /// Geographical position.
    pub position: Position,

    /// Text displayed next to the marker.
    pub label: String,

    /// Symbol drawn on the place. You can check [egui's font book](https://www.egui.rs/) to pick
    /// a proper character.
    pub symbol: char,

    /// Visual style of this place.
    pub style: Style,
}

/// [`Plugin`] which draws list of places on the map.
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

            let label = painter.layout_no_wrap(
                place.label.to_owned(),
                place.style.label_font.clone(),
                place.style.label_text_color,
            );

            // Offset of the label, relative to the circle.
            let offset = vec2(8., 8.);

            painter.rect_filled(
                label
                    .rect
                    .translate(screen_position)
                    .translate(offset)
                    .expand(5.),
                10.,
                place.style.label_background,
            );

            painter.galley((screen_position + offset).to_pos2(), label);

            painter.circle(
                screen_position.to_pos2(),
                10.,
                place.style.symbol_fill_color,
                place.style.symbol_stroke,
            );

            painter.text(
                screen_position.to_pos2(),
                Align2::CENTER_CENTER,
                place.symbol.to_string(),
                place.style.symbol_font.clone(),
                place.style.symbol_color,
            );
        }
    }
}
