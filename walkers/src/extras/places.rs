use egui::{vec2, Align2, Color32, FontId, Response, Stroke, Ui};

use crate::{Plugin, Position};

/// Visual style of the place.
#[derive(Clone)]
pub struct Style {
    pub label_font: FontId,
    pub label_color: Color32,
    pub label_background: Color32,
    pub symbol_font: FontId,
    pub symbol_color: Color32,
    pub symbol_background: Color32,
    pub symbol_stroke: Stroke,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            label_font: FontId::proportional(12.),
            label_color: Color32::from_gray(200),
            label_background: Color32::BLACK.gamma_multiply(0.8),
            symbol_font: FontId::proportional(14.),
            symbol_color: Color32::BLACK.gamma_multiply(0.8),
            symbol_background: Color32::WHITE.gamma_multiply(0.8),
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

impl Place {
    fn draw(&self, ui: &Ui, projector: &crate::Projector) {
        let screen_position = projector.project(self.position);
        let painter = ui.painter();

        let label = painter.layout_no_wrap(
            self.label.to_owned(),
            self.style.label_font.clone(),
            self.style.label_color,
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
            self.style.label_background,
        );

        painter.galley((screen_position + offset).to_pos2(), label, Color32::BLACK);

        painter.circle(
            screen_position.to_pos2(),
            10.,
            self.style.symbol_background,
            self.style.symbol_stroke,
        );

        painter.text(
            screen_position.to_pos2(),
            Align2::CENTER_CENTER,
            self.symbol.to_string(),
            self.style.symbol_font.clone(),
            self.style.symbol_color,
        );
    }
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
    fn run(self: Box<Self>, ui: &mut Ui, _response: &Response, projector: &crate::Projector) {
        for place in &self.places {
            place.draw(ui, projector);
        }
    }
}
