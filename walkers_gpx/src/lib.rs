use egui::{vec2, Align2, Color32, FontId, PointerButton, Stroke, Vec2};
use egui::{Painter, Response};
use gpx::errors::GpxError;
use gpx::Gpx;
use std::io::Read;
use walkers::{Plugin, Position, Projector};

pub struct WalkerGpx {
    gpx: Gpx,
    style: Style,
    select: Option<GpxIndex>,
}

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

impl WalkerGpx {
    pub fn read<R: Read>(reader: R) -> Result<WalkerGpx, GpxError> {
        Ok(WalkerGpx {
            gpx: gpx::read(reader)?,
            style: Default::default(),
            select: Default::default(),
        })
    }
}

#[derive(Default, Eq, PartialEq, Clone, Copy, Debug)]
struct GpxIndex {
    track: usize,
    segment: usize,
    waypoint: usize,
}

impl Plugin for &mut WalkerGpx {
    fn run(&mut self, response: &Response, painter: Painter, projector: &Projector) {
        let clicked_at_screen =
            if !response.changed() && response.clicked_by(egui::PointerButton::Primary) {
                response.interact_pointer_pos()
            } else {
                None
            };

        let at_screen = painter.ctx().pointer_latest_pos();

        dbg!(&at_screen, &self.select);

        let mut current_index = GpxIndex::default();

        for (i, track) in self.gpx.tracks.iter().enumerate() {
            current_index.track = i;
            for (i, segment) in track.segments.iter().enumerate() {
                current_index.segment = i;
                let mut prev_screen_position: Option<_> = None::<Vec2>;
                for (i, waypoint) in segment.points.iter().enumerate() {
                    current_index.waypoint = i;

                    let position = self
                        .select
                        .clone()
                        .filter(|&a| a == current_index)
                        .and(at_screen)
                        .map(|p| dbg!(projector.unproject(p - response.rect.center())))
                        .unwrap_or(Position::from_lon_lat(
                            waypoint.point().x(),
                            waypoint.point().y(),
                        ));

                    // let position =
                    //     Position::from_lon_lat(waypoint.point().x(), waypoint.point().y());
                    let screen_position = projector.project(position);

                    let radius = 10.;

                    let hovered = response
                        .hover_pos()
                        .map(|hover_pos| hover_pos.distance(screen_position.to_pos2()) < radius)
                        .unwrap_or(false);

                    painter.circle(
                        screen_position.to_pos2(),
                        10.,
                        //     self.style.symbol_background,
                        Color32::BLACK.gamma_multiply(if hovered { 0.5 } else { 0.2 }),
                        self.style.symbol_stroke,
                    );

                    if let Some(clicked_at_screen) = clicked_at_screen {
                        if clicked_at_screen.distance(screen_position.to_pos2()) < radius {
                            self.select = Some(current_index.clone());
                            println!(" Select {:?}", self.select)
                        }
                    }

                    if let Some(prev_screen_position) = prev_screen_position {
                        painter.line_segment(
                            [prev_screen_position.to_pos2(), screen_position.to_pos2()],
                            self.style.symbol_stroke,
                        );
                    }

                    prev_screen_position = Some(screen_position);
                }
            }
        }
    }
}

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
