use crate::{
    Position,
    position::{halfway, pixel_distance, shifted},
};
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
    Exact(Position),

    /// Map is being dragged by mouse or finger.
    Moving {
        position: Position,
        velocity: Vec2,
        /// Whether the drag was started from a detached state.
        from_detached: bool,
    },

    /// Map is moving, but due to inertia, and will slow down and stop in a short while.
    Inertia { position: Position, velocity: Vec2 },

    /// Map is being pulled back to the `my_position`. This happens when the user releases the
    /// dragging gesture, but the map is too close to the `my_position`.
    PulledToMyPosition(Position),
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
            self.drag_stopped(my_position, pull_to_my_position_threshold, zoom);
            true
        } else {
            false
        }
    }

    fn dragged_by(&mut self, my_position: Position, response: &Response, zoom: f64) {
        let from_detached = if let Center::Moving { from_detached, .. } = self {
            *from_detached
        } else {
            self.detached().is_some()
        };

        *self = Center::Moving {
            position: shifted(self.position(my_position), response.drag_delta(), zoom),
            velocity: pointer_velocity(response) * INERTIA_MULTIPLIER,
            from_detached,
        };
    }

    fn drag_stopped(
        &mut self,
        my_position: Position,
        pull_to_my_position_threshold: f32,
        zoom: f64,
    ) {
        if let Center::Moving {
            position,
            velocity,
            from_detached,
        } = *self
        {
            if from_detached
                || pixel_distance(position, my_position, zoom) > pull_to_my_position_threshold
            {
                *self = Center::Inertia { position, velocity };
            } else {
                *self = Center::PulledToMyPosition(position);
            }
        }
    }

    pub(crate) fn update_movement(
        &mut self,
        delta_time: f32,
        my_position: Position,
        zoom: f64,
    ) -> bool {
        match *self {
            Center::Inertia { position, velocity } => {
                // Exponentially drive the velocity towards zero.
                let lp_factor = INERTIA_TAU / (delta_time + INERTIA_TAU);
                let velocity = velocity * lp_factor;

                *self = if velocity.length() < INERTIA_STOP_SPEED {
                    Center::Exact(position)
                } else {
                    Center::Inertia {
                        position: shifted(position, velocity * delta_time, zoom),
                        velocity,
                    }
                };
                true
            }
            Center::PulledToMyPosition(position) => {
                // Close half of the distance on each frame.
                let position = halfway(position, my_position, zoom);
                *self = if pixel_distance(position, my_position, zoom) < 1.0 {
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
        match *self {
            Center::MyPosition => None,
            Center::Exact(position)
            | Center::PulledToMyPosition(position)
            | Center::Moving { position, .. }
            | Center::Inertia { position, .. } => Some(position),
        }
    }

    pub fn animating(&self) -> bool {
        matches!(self, Center::Inertia { .. } | Center::PulledToMyPosition(_))
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
                Center::PulledToMyPosition(shifted(position, offset, zoom))
            }
            Center::Exact(position) => Center::Exact(shifted(position, offset, zoom)),
            Center::Moving {
                position,
                velocity,
                from_detached,
            } => Center::Moving {
                position: shifted(position, offset, zoom),
                velocity,
                from_detached,
            },
            Center::Inertia { position, velocity } => Center::Inertia {
                position: shifted(position, offset, zoom),
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
        let mut center = Center::Exact(position);
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

        let dragged = distance_from(&center, position);

        for _ in 0..10000 {
            if !center.update_movement(frame_time, Position::default(), ZOOM) {
                break;
            }
        }

        distance_from(&center, position) - dragged
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

    /// How far, in pixels, the map has moved from `start`.
    fn distance_from(center: &Center, start: Position) -> f32 {
        center
            .detached()
            .map(|position| pixel_distance(position, start, ZOOM))
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
