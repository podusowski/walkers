use egui::{Response, Vec2};

use crate::{
    projector::Projector,
    units::{AdjustedPosition, Position},
};

/// Position at the map's center. Initially, the map follows `my_position` argument which typically
/// is meant to be fed by a GPS sensor or other geo-localization method. If user drags the map,
/// it becomes "detached" and stays this way until [`MapMemory::center_mode`] is changed back to
/// [`Center::MyPosition`].
#[derive(Clone, Default)]
pub(crate) enum Center {
    /// Centered at `my_position` argument of the [`Map::new()`] function.
    #[default]
    MyPosition,

    /// Centered exactly here
    Exact { adjusted_pos: AdjustedPosition },

    /// Map is currently being dragged.
    Moving {
        adjusted_pos: AdjustedPosition,
        direction: Vec2,
    },

    /// Map is currently moving due to inertia, and will slow down and stop after a short while.
    Inertia {
        adjusted_pos: AdjustedPosition,
        direction: Vec2,
        amount: f32,
    },
}

impl Center {
    pub(crate) fn recalculate_drag(&mut self, response: &Response, my_position: Position) -> bool {
        if response.dragged_by(egui::PointerButton::Primary) {
            *self = Center::Moving {
                adjusted_pos: self
                    .adjusted_position()
                    .unwrap_or(AdjustedPosition::new(my_position, Default::default())),
                direction: response.drag_delta(),
            };
            true
        } else if response.drag_stopped() {
            if let Center::Moving {
                adjusted_pos,
                direction,
            } = &self
            {
                *self = Center::Inertia {
                    adjusted_pos: adjusted_pos.to_owned(),
                    direction: *direction,
                    amount: 1.0,
                };
            }
            true
        } else {
            false
        }
    }

    pub(crate) fn update_movement(&mut self) -> bool {
        match self {
            Center::Moving {
                adjusted_pos,
                direction,
            } => {
                let delta = *direction;

                *adjusted_pos = adjusted_pos.to_owned().shift(delta);

                true
            }
            Center::Inertia {
                adjusted_pos,
                direction,
                amount,
            } => {
                if amount <= &mut 0.0 {
                    *self = Center::Exact {
                        adjusted_pos: adjusted_pos.to_owned(),
                    }
                } else {
                    let delta = *direction * *amount;

                    *adjusted_pos = adjusted_pos.to_owned().shift(delta);
                };
                true
            }
            _ => false,
        }
    }

    /// Returns exact position if map is detached (i.e. not following `my_position`),
    /// `None` otherwise.
    pub(crate) fn detached(&self, projector: &Projector) -> Option<Position> {
        self.adjusted_position().map(|p| projector.position(p))
    }

    /// Get the real position at the map's center.
    pub fn position(&self, my_position: Position, projector: &Projector) -> Position {
        self.detached(projector).unwrap_or(my_position)
    }

    pub(crate) fn adjusted_position(&self) -> Option<AdjustedPosition> {
        match self {
            Center::MyPosition => None,
            Center::Exact { adjusted_pos }
            | Center::Moving { adjusted_pos, .. }
            | Center::Inertia { adjusted_pos, .. } => Some(adjusted_pos.to_owned()),
        }
    }

    /// Shift position by given number of pixels, if detached.
    pub(crate) fn shift(self, shift_offset: Vec2) -> Self {
        match self {
            Center::MyPosition => Center::MyPosition,
            Center::Exact { adjusted_pos } => Center::Exact {
                adjusted_pos: adjusted_pos.shift(shift_offset),
            },
            Center::Moving {
                adjusted_pos,
                direction,
            } => Center::Moving {
                adjusted_pos: adjusted_pos.shift(shift_offset),
                direction,
            },
            Center::Inertia {
                adjusted_pos,
                direction,
                amount,
            } => Center::Inertia {
                adjusted_pos: adjusted_pos.shift(shift_offset),
                direction,
                amount,
            },
        }
    }

    pub fn zero_offset(self, projector: &Projector) -> Self {
        match self {
            Center::MyPosition => Center::MyPosition,
            Center::Exact { adjusted_pos } => Center::Exact {
                adjusted_pos: projector.zero_offset(adjusted_pos),
            },
            Center::Moving {
                adjusted_pos,
                direction,
            } => Center::Moving {
                adjusted_pos: projector.zero_offset(adjusted_pos),
                direction,
            },
            Center::Inertia {
                adjusted_pos,
                direction,
                amount,
            } => Center::Inertia {
                adjusted_pos: projector.zero_offset(adjusted_pos),
                direction,
                amount,
            },
        }
    }
}
