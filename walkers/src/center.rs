use crate::{Position, position::AdjustedPosition};
use egui::{DragPanButtons, PointerButton, Response, Vec2};

/// Time constant of inertia stopping filter
const INERTIA_TAU: f32 = 0.2f32;

/// Speed, in points per second, below which the inertia is considered to be over.
const INERTIA_STOP_SPEED: f32 = 10f32;

/// Multiplier for the inertia velocity, to make it feel more natural.
const INERTIA_MULTIPLIER: f32 = 2.0;

/// Position of the map's center. Initially, the map follows `my_position` argument which typically
/// is meant to be fed by a GPS sensor or other geo-localization method. If user drags the map,
/// it becomes "detached" and stays this way until [`MapMemory::center_mode`] is changed back to
/// [`Center::MyPosition`].
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub(crate) enum Center {
    /// Centered at `my_position` argument of the [`Map::new()`] function.
    #[default]
    MyPosition,

    /// Centered at the exact position.
    Exact(AdjustedPosition),

    /// Map is being dragged by mouse or finger.
    Moving {
        position: AdjustedPosition,
        velocity: Vec2,
        /// Whether the drag was started from a detached state.
        from_detached: bool,
    },

    /// Map is moving, but due to inertia, and will slow down and stop in a short while.
    Inertia {
        position: AdjustedPosition,
        velocity: Vec2,
    },

    /// Map is being pulled back to the `my_position`. This happens when the user releases the
    /// dragging gesture, but the map is too close to the `my_position`.
    PulledToMyPosition(AdjustedPosition),
}

impl Center {
    pub(crate) fn handle_gestures(
        &mut self,
        response: &Response,
        my_position: Position,
        pull_to_my_position_threshold: f32,
        drag_pan_buttons: DragPanButtons,
        zoom: f64,
    ) -> bool {
        if dragged_by(response, drag_pan_buttons) {
            self.dragged_by(my_position, response, zoom);
            true
        } else if response.drag_stopped() {
            self.drag_stopped(pull_to_my_position_threshold);
            true
        } else {
            false
        }
    }

    fn dragged_by(&mut self, my_position: Position, response: &Response, zoom: f64) {
        let from_detached = if let Center::Moving { from_detached, .. } = self {
            *from_detached
        } else {
            // Only `MyPosition` state has no adjusted position.
            self.adjusted_position().is_some()
        };

        *self = Center::Moving {
            position: self
                .adjusted_position()
                .unwrap_or(AdjustedPosition::new(my_position))
                .shift(response.drag_delta(), zoom),
            velocity: pointer_velocity(response) * INERTIA_MULTIPLIER,
            from_detached,
        };
    }

    fn drag_stopped(&mut self, pull_to_my_position_threshold: f32) {
        if let Center::Moving {
            position,
            velocity,
            from_detached,
        } = &self
        {
            if *from_detached || position.offset_length() > pull_to_my_position_threshold {
                *self = Center::Inertia {
                    position: position.clone(),
                    velocity: *velocity,
                };
            } else {
                *self = Center::PulledToMyPosition(position.to_owned());
            }
        }
    }

    pub(crate) fn update_movement(&mut self, delta_time: f32, zoom: f64) -> bool {
        match &self {
            Center::Inertia { position, velocity } => {
                // Exponentially drive the velocity towards zero.
                let lp_factor = INERTIA_TAU / (delta_time + INERTIA_TAU);
                let velocity = *velocity * lp_factor;

                *self = if velocity.length() < INERTIA_STOP_SPEED {
                    Center::Exact(position.to_owned())
                } else {
                    Center::Inertia {
                        position: position.clone().shift(velocity * delta_time, zoom),
                        velocity,
                    }
                };
                true
            }
            Center::PulledToMyPosition(position) => {
                let position = position.clone().half_offset();
                *self = if position.offset_length() < 1.0 {
                    Center::MyPosition
                } else {
                    Center::PulledToMyPosition(position)
                };
                true
            }
            _ => false,
        }
    }

    /// Returns exact position if map is detached (i.e. not following `my_position`),
    /// `None` otherwise.
    pub(crate) fn detached(&self) -> Option<Position> {
        self.adjusted_position().map(|p| p.position())
    }

    pub fn animating(&self) -> bool {
        matches!(self, Center::Inertia { .. } | Center::PulledToMyPosition(_))
    }

    fn adjusted_position(&self) -> Option<AdjustedPosition> {
        match self {
            Center::MyPosition => None,
            Center::Exact(position)
            | Center::PulledToMyPosition(position)
            | Center::Moving { position, .. }
            | Center::Inertia { position, .. } => Some(position.to_owned()),
        }
    }

    /// Get the real position at the map's center.
    pub fn position(&self, my_position: Position) -> Position {
        self.detached().unwrap_or(my_position)
    }

    /// Shift position by given number of pixels, if detached.
    pub(crate) fn shift(self, offset: Vec2, zoom: f64) -> Self {
        match self {
            Center::MyPosition => Center::MyPosition,
            Center::PulledToMyPosition(position) => {
                Center::PulledToMyPosition(position.shift(offset, zoom))
            }
            Center::Exact(position) => Center::Exact(position.shift(offset, zoom)),
            Center::Moving {
                position,
                velocity,
                from_detached,
            } => Center::Moving {
                position: position.shift(offset, zoom),
                velocity,
                from_detached,
            },
            Center::Inertia { position, velocity } => Center::Inertia {
                position: position.shift(offset, zoom),
                velocity,
            },
        }
    }
}

fn dragged_by(response: &Response, buttons: DragPanButtons) -> bool {
    buttons.iter().any(|button| match button {
        DragPanButtons::PRIMARY => response.dragged_by(PointerButton::Primary),
        DragPanButtons::SECONDARY => response.dragged_by(PointerButton::Secondary),
        DragPanButtons::MIDDLE => response.dragged_by(PointerButton::Middle),
        DragPanButtons::EXTRA_1 => response.dragged_by(PointerButton::Extra1),
        DragPanButtons::EXTRA_2 => response.dragged_by(PointerButton::Extra2),
        _ => false,
    })
}

/// How fast the pointer moves, in points per second.
fn pointer_velocity(response: &Response) -> Vec2 {
    let (velocity, delta_time) = response
        .ctx
        .input(|input| (input.pointer.velocity(), input.stable_dt));

    if velocity == Vec2::ZERO && delta_time > 0. {
        // `egui` gives up when the frame rate is poor, so measure it ourselves.
        response.drag_delta() / delta_time
    } else {
        velocity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lat_lon;
    use approx::assert_relative_eq;

    const ZOOM: f64 = 16.;

    /// Drag the map with a constant pointer speed, let it go, and return how far the map moved.
    fn fling_distance(pointer_speed: f32, frame_time: f32) -> f32 {
        let ctx = egui::Context::default();
        let position = lat_lon(51.10, 17.03);
        let mut center = Center::Exact(AdjustedPosition::new(position));
        let mut pointer = egui::pos2(100., 100.);

        // The whole gesture, frame by frame. The first one is empty, because `egui` needs to know
        // about the widget before the pointer can grab it.
        let mut frames = vec![Vec::new(), vec![pointer_button(pointer, true)]];

        for _ in 0..10 {
            pointer.x += pointer_speed * frame_time;
            frames.push(vec![egui::Event::PointerMoved(pointer)]);
        }

        frames.push(vec![pointer_button(pointer, false)]);

        for (frame, events) in frames.into_iter().enumerate() {
            let time = frame as f64 * frame_time as f64;
            run_frame(&ctx, &mut center, events, time, frame_time);
        }

        let dragged = offset_length(&center);

        for _ in 0..10000 {
            if !center.update_movement(frame_time, ZOOM) {
                break;
            }
        }

        offset_length(&center) - dragged
    }

    /// Run a single frame of a map being dragged, letting `center` handle the gestures.
    fn run_frame(
        ctx: &egui::Context,
        center: &mut Center,
        events: Vec<egui::Event>,
        time: f64,
        frame_time: f32,
    ) {
        let mut output = ctx.run_ui(raw_input(events, time, frame_time), |ui| {
            let (_, response) =
                ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

            // Map is detached, so `my_position` does not matter.
            center.handle_gestures(
                &response,
                Position::default(),
                0.,
                DragPanButtons::PRIMARY,
                ZOOM,
            );
        });

        // `egui` insists on these being handled before they are dropped.
        output.textures_delta.clear();
    }

    fn raw_input(events: Vec<egui::Event>, time: f64, frame_time: f32) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(2000., 500.),
            )),
            time: Some(time),
            predicted_dt: frame_time,
            events,
            ..Default::default()
        }
    }

    fn pointer_button(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }

    fn offset_length(center: &Center) -> f32 {
        center
            .adjusted_position()
            .map(|position| position.offset_length())
            .unwrap_or_default()
    }

    #[test]
    fn fling_does_not_depend_on_how_long_the_frames_take() {
        let pointer_speed = 1000.;
        let expected = pointer_speed * INERTIA_MULTIPLIER * INERTIA_TAU;

        assert_relative_eq!(
            fling_distance(pointer_speed, 1. / 60.),
            expected,
            max_relative = 0.05
        );
        assert_relative_eq!(
            fling_distance(pointer_speed, 1. / 10.),
            expected,
            max_relative = 0.05
        );
    }
}
