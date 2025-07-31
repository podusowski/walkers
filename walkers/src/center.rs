use crate::{position::AdjustedPosition, Position};
use egui::{DragPanButtons, PointerButton, Response, Vec2};

/// Time constant of inertia stopping filter
const INERTIA_TAU: f32 = 0.2f32;

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
        direction: Vec2,
        /// Whether the drag was started from a detached state.
        from_detached: bool,
    },

    /// Map is moving, but due to inertia, and will slow down and stop in a short while.
    Inertia {
        position: AdjustedPosition,
        direction: Vec2,
        amount: f32,
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
    ) -> bool {
        if dragged_by(response, drag_pan_buttons) {
            self.dragged_by(my_position, response);
            true
        } else if response.drag_stopped() {
            self.drag_stopped(pull_to_my_position_threshold);
            true
        } else {
            false
        }
    }

    fn dragged_by(&mut self, my_position: Position, response: &Response) {
        let from_detached = if let Center::Moving { from_detached, .. } = self {
            *from_detached
        } else {
            // Only `MyPosition` state has no adjusted position.
            self.adjusted_position().is_some()
        };

        *self = Center::Moving {
            position: self
                .adjusted_position()
                .unwrap_or(AdjustedPosition::new(my_position)),
            direction: response.drag_delta(),
            from_detached,
        };
    }

    fn drag_stopped(&mut self, pull_to_my_position_threshold: f32) {
        if let Center::Moving {
            position,
            direction,
            from_detached,
        } = &self
        {
            if *from_detached || position.offset_length() > pull_to_my_position_threshold {
                *self = Center::Inertia {
                    position: position.clone(),
                    direction: direction.normalized(),
                    amount: direction.length(),
                };
            } else {
                *self = Center::PulledToMyPosition(position.to_owned());
            }
        }
    }

    pub(crate) fn update_movement(&mut self, delta_time: f32, zoom: f64) -> bool {
        match &self {
            Center::Moving {
                position,
                direction,
                from_detached,
            } => {
                *self = Center::Moving {
                    position: position.clone().shift(*direction, zoom),
                    direction: *direction,
                    from_detached: *from_detached,
                };
                true
            }
            Center::Inertia {
                position,
                direction,
                amount,
            } => {
                *self = if amount < &mut 0.1 {
                    Center::Exact(position.to_owned())
                } else {
                    // Exponentially drive the `amount` value towards zero
                    let lp_factor = INERTIA_TAU / (delta_time + INERTIA_TAU);

                    Center::Inertia {
                        position: position.clone().shift(*direction * *amount, zoom),
                        direction: *direction,
                        amount: *amount * lp_factor,
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
                direction,
                from_detached,
            } => Center::Moving {
                position: position.shift(offset, zoom),
                direction,
                from_detached,
            },
            Center::Inertia {
                position,
                direction,
                amount,
            } => Center::Inertia {
                position: position.shift(offset, zoom),
                direction,
                amount,
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
