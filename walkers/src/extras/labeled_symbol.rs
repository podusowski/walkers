use super::places::{Group, Place};
use crate::{Position, Projector};
use egui::{vec2, Align2, Color32, FontId, Stroke, Ui};

#[derive(Clone)]
/// Type of the symbol of a [`LabeledSymbol`].
pub enum Symbol {
    Circle(String),
    TwoCorners(String),
}

/// A symbol with a label to be drawn on the map.
#[derive(Clone)]
pub struct LabeledSymbol {
    /// Geographical position.
    pub position: Position,

    /// Text displayed next to the marker.
    pub label: String,

    /// Symbol drawn on the place. You can check [egui's font book](https://www.egui.rs/) to pick
    /// a desired character.
    pub symbol: Option<Symbol>,

    /// Visual style of this place.
    pub style: LabeledSymbolStyle,
}

impl Place for LabeledSymbol {
    fn position(&self) -> Position {
        self.position
    }

    fn draw(&self, ui: &Ui, projector: &crate::Projector) {
        let screen_position = projector.project(self.position);
        let painter = ui.painter();

        self.draw_label(painter, screen_position);

        match self.symbol {
            Some(Symbol::Circle(ref text)) => {
                self.draw_circle_symbol(text.clone(), painter, screen_position.to_pos2())
            }
            Some(Symbol::TwoCorners(ref text)) => {
                self.draw_two_corners_symbol(text.clone(), painter, screen_position.to_pos2())
            }
            None => {}
        }
    }
}

impl LabeledSymbol {
    fn draw_circle_symbol(
        &self,
        text: String,
        painter: &egui::Painter,
        screen_position: egui::Pos2,
    ) {
        painter.circle(
            screen_position,
            self.style.symbol_size / 2.,
            self.style.symbol_background,
            self.style.symbol_stroke,
        );

        painter.text(
            screen_position,
            Align2::CENTER_CENTER,
            text,
            self.style.symbol_font.clone(),
            self.style.symbol_color,
        );
    }

    fn draw_two_corners_symbol(
        &self,
        text: String,
        painter: &egui::Painter,
        screen_position: egui::Pos2,
    ) {
        let half_size = self.style.symbol_size / 2.;
        let top_left = screen_position + vec2(-half_size, -half_size);
        let bottom_right = screen_position + vec2(half_size, half_size);
        let top_right = screen_position + vec2(half_size, -half_size);
        let bottom_left = screen_position + vec2(-half_size, half_size);
        let len = 4.;

        // Background rectangle.
        painter.rect_filled(
            egui::Rect::from_min_max(top_left, bottom_right),
            0.,
            self.style.symbol_background,
        );

        // Top right.
        painter.line_segment(
            [top_right, top_right + vec2(-len, 0.)],
            self.style.symbol_stroke,
        );
        painter.line_segment(
            [top_right, top_right + vec2(0., len)],
            self.style.symbol_stroke,
        );

        // Bottom left.
        painter.line_segment(
            [bottom_left, bottom_left + vec2(len, 0.)],
            self.style.symbol_stroke,
        );
        painter.line_segment(
            [bottom_left, bottom_left + vec2(0., -len)],
            self.style.symbol_stroke,
        );

        // Text.
        painter.text(
            screen_position,
            Align2::CENTER_CENTER,
            text.clone(),
            self.style.symbol_font.clone(),
            self.style.symbol_color,
        );
    }

    fn draw_label(&self, painter: &egui::Painter, screen_position: egui::Vec2) {
        let label = painter.layout_no_wrap(
            self.label.to_owned(),
            self.style.label_font.clone(),
            self.style.label_color,
        );

        // Offset of the label, relative to the circle.
        let offset = vec2(8., 8.);

        // Label background.
        painter.rect_filled(
            label
                .rect
                .translate(screen_position)
                .translate(offset)
                .expand(5.),
            self.style.label_corner_radius,
            self.style.label_background,
        );

        painter.galley((screen_position + offset).to_pos2(), label, Color32::BLACK);
    }
}

/// Visual style of a [`LabeledSymbol`].
#[derive(Clone)]
pub struct LabeledSymbolStyle {
    pub label_font: FontId,
    pub label_color: Color32,
    pub label_background: Color32,
    pub label_corner_radius: f32,
    pub symbol_font: FontId,
    pub symbol_color: Color32,
    pub symbol_background: Color32,
    pub symbol_stroke: Stroke,
    pub symbol_size: f32,
}

impl Default for LabeledSymbolStyle {
    fn default() -> Self {
        Self {
            label_font: FontId::proportional(12.),
            label_color: Color32::from_gray(200),
            label_background: Color32::BLACK.gamma_multiply(0.8),
            label_corner_radius: 10.,
            symbol_font: FontId::proportional(14.),
            symbol_color: Color32::BLACK.gamma_multiply(0.8),
            symbol_background: Color32::WHITE.gamma_multiply(0.8),
            symbol_stroke: Stroke::new(2., Color32::BLACK.gamma_multiply(0.8)),
            symbol_size: 10.,
        }
    }
}

pub struct LabeledSymbolGroup {
    pub style: LabeledSymbolGroupStyle,
}

impl Group for LabeledSymbolGroup {
    fn draw<T: Place>(
        &self,
        places: &[&T],
        position: Position,
        projector: &Projector,
        ui: &mut Ui,
    ) {
        let screen_position = projector.project(position);
        let painter = ui.painter();

        painter.circle(
            screen_position.to_pos2(),
            10.,
            self.style.background,
            self.style.stroke,
        );

        painter.text(
            screen_position.to_pos2(),
            Align2::CENTER_CENTER,
            format!("{}", places.len()),
            self.style.font.clone(),
            self.style.color,
        );
    }
}

/// Visual style of a [`LabeledSymbolGroup`].
#[derive(Clone)]
pub struct LabeledSymbolGroupStyle {
    pub font: FontId,
    pub color: Color32,
    pub background: Color32,
    pub stroke: Stroke,
}

impl Default for LabeledSymbolGroupStyle {
    fn default() -> Self {
        Self {
            font: FontId::proportional(12.),
            color: Color32::WHITE.gamma_multiply(0.8),
            background: Color32::BLACK.gamma_multiply(0.8),
            stroke: Stroke::new(2., Color32::BLACK.gamma_multiply(0.8)),
        }
    }
}
