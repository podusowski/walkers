use egui::{Response, Vec2};

use crate::{
    position::{AdjustedPosition, Pixels},
    Position,
};

/// Time constant of inertia stopping filter
const INTERTIA_TAU: f32 = 0.2f32;

/// Position at the map's center. Initially, the map follows `my_position` argument which typically
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

    /// Map is currently being dragged.
    Moving {
        position: AdjustedPosition,
        direction: Vec2,
    },

    /// Map is currently moving due to inertia, and will slow down and stop after a short while.
    Inertia {
        position: AdjustedPosition,
        direction: Vec2,
        amount: f32,
    },
}

impl Center {
    pub(crate) fn recalculate_drag(&mut self, response: &Response, my_position: Position) -> bool {
        if response.dragged_by(egui::PointerButton::Primary) {
            *self = Center::Moving {
                position: self
                    .adjusted_position()
                    .unwrap_or(AdjustedPosition::new(my_position, Default::default())),
                direction: response.drag_delta(),
            };
            true
        } else if response.drag_stopped() {
            if let Center::Moving {
                position,
                direction,
            } = &self
            {
                *self = Center::Inertia {
                    position: position.clone(),
                    direction: direction.normalized(),
                    amount: direction.length(),
                };
            }
            true
        } else {
            false
        }
    }

    pub(crate) fn update_movement(&mut self, delta_time: f32) -> bool {
        match &self {
            Center::Moving {
                position,
                direction,
            } => {
                let delta = *direction;
                let offset = position.offset + Pixels::new(delta.x as f64, delta.y as f64);

                *self = Center::Moving {
                    position: AdjustedPosition::new(position.position, offset),
                    direction: *direction,
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
                    let delta = *direction * *amount;
                    let offset = position.offset + Pixels::new(delta.x as f64, delta.y as f64);

                    // Exponentially drive the `amount` value towards zero
                    let lp_factor = INTERTIA_TAU / (delta_time + INTERTIA_TAU);

                    Center::Inertia {
                        position: AdjustedPosition::new(position.position, offset),
                        direction: *direction,
                        amount: *amount * lp_factor,
                    }
                };
                true
            }
            _ => false,
        }
    }

    /// Returns exact position if map is detached (i.e. not following `my_position`),
    /// `None` otherwise.
    pub(crate) fn detached(&self, zoom: f64) -> Option<Position> {
        self.adjusted_position().map(|p| p.position(zoom))
    }

    fn adjusted_position(&self) -> Option<AdjustedPosition> {
        match self {
            Center::MyPosition => None,
            Center::Exact(position)
            | Center::Moving { position, .. }
            | Center::Inertia { position, .. } => Some(position.to_owned()),
        }
    }

    /// Get the real position at the map's center.
    pub fn position(&self, my_position: Position, zoom: f64) -> Position {
        self.detached(zoom).unwrap_or(my_position)
    }

    pub fn zero_offset(self, zoom: f64) -> Self {
        match self {
            Center::MyPosition => Center::MyPosition,
            Center::Exact(position) => Center::Exact(position.zero_offset(zoom)),
            Center::Moving {
                position,
                direction,
            } => Center::Moving {
                position: position.zero_offset(zoom),
                direction,
            },
            Center::Inertia {
                position,
                direction,
                amount,
            } => Center::Inertia {
                position: position.zero_offset(zoom),
                direction,
                amount,
            },
        }
    }

    /// Shift position by given number of pixels, if detached.
    pub(crate) fn shift(self, offset: Vec2) -> Self {
        match self {
            Center::MyPosition => Center::MyPosition,
            Center::Exact(position) => Center::Exact(position.shift(offset)),
            Center::Moving {
                position,
                direction,
            } => Center::Moving {
                position: position.shift(offset),
                direction,
            },
            Center::Inertia {
                position,
                direction,
                amount,
            } => Center::Inertia {
                position: position.shift(offset),
                direction,
                amount,
            },
        }
    }
}
