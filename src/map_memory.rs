use crate::{
    center::Center,
    units::{AdjustedPosition, Position},
    zoom::{InvalidZoom, Zoom},
    Projector,
};

/// State of the map widget which must persist between frames.
#[derive(Default, Clone)]
pub struct MapMemory {
    pub(crate) center_mode: Center,
    pub(crate) zoom: Zoom,
}

impl MapMemory {
    /// Returns the current zoom level
    pub fn zoom(&self) -> f64 {
        self.zoom.into()
    }

    pub fn zoom_in(&mut self, projector: &impl Projector) -> Result<(), InvalidZoom> {
        self.center_mode = self.center_mode.clone().zero_offset(projector);
        self.zoom.zoom_in()
    }

    /// Try to zoom out, returning `Err(InvalidZoom)` if already at minimum.
    pub fn zoom_out(&mut self, projector: &impl Projector) -> Result<(), InvalidZoom> {
        self.center_mode = self.center_mode.clone().zero_offset(projector);
        self.zoom.zoom_out()
    }

    /// Set exact zoom level
    pub fn set_zoom(&mut self, zoom: f64, projector: &impl Projector) -> Result<(), InvalidZoom> {
        self.center_mode = self.center_mode.clone().zero_offset(projector);
        self.zoom = Zoom::try_from(zoom)?;
        Ok(())
    }

    /// Returns exact position if map is detached (i.e. not following `my_position`),
    /// `None` otherwise.
    pub fn detached(&self, projector: &impl Projector) -> Option<Position> {
        self.center_mode.detached(projector)
    }

    /// Center exactly at the given position.
    pub fn center_at(&mut self, pos: Position) {
        self.center_mode = Center::Exact {
            adjusted_pos: AdjustedPosition::new(pos, Default::default()),
        };
    }

    /// Follow `my_position`.
    pub fn follow_my_position(&mut self) {
        self.center_mode = Center::MyPosition;
    }
}
