use crate::{InvalidZoom, Position, center::Center, position::AdjustedPosition, zoom::Zoom};

/// State of the map widget which must persist between frames.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub struct MapMemory {
    pub(crate) center_mode: Center,
    pub(crate) zoom: Zoom,
    /// Rotation angle in radians (clockwise)
    pub(crate) rotation: f32,
}

impl MapMemory {
    /// Try to zoom in, returning `Err(InvalidZoom)` if already at maximum.
    pub fn zoom_in(&mut self) -> Result<(), InvalidZoom> {
        self.zoom.zoom_in()
    }

    /// Try to zoom out, returning `Err(InvalidZoom)` if already at minimum.
    pub fn zoom_out(&mut self) -> Result<(), InvalidZoom> {
        self.zoom.zoom_out()
    }

    /// Set exact zoom level
    pub fn set_zoom(&mut self, zoom: f64) -> Result<(), InvalidZoom> {
        self.zoom = Zoom::try_from(zoom)?;
        Ok(())
    }

    /// Returns the current zoom level
    pub fn zoom(&self) -> f64 {
        self.zoom.into()
    }

    /// If the map is in detached state, returns the geographical position
    /// of the center. `None` if the map is not detached, i.e. following
    /// `my_position`.
    pub fn detached(&self) -> Option<Position> {
        self.center_mode.detached()
    }

    /// Whether the map is currently animating. Dragging, zooming and `my_position` changes are not
    /// considered animation.
    pub fn animating(&self) -> bool {
        self.center_mode.animating()
    }

    /// Point the map exactly at the given geographical position.
    pub fn center_at(&mut self, position: Position) {
        self.center_mode = Center::Exact(AdjustedPosition::new(position));
    }

    /// Start following `my_position` given in [`crate::Map::new`].
    pub fn follow_my_position(&mut self) {
        self.center_mode = Center::MyPosition;
    }

    /// Set the map rotation angle in radians (clockwise)
    pub fn set_rotation(&mut self, rotation: f32) {
        self.rotation = rotation;
    }

    /// Get the map rotation angle in radians (clockwise)
    pub fn rotation(&self) -> f32 {
        self.rotation
    }
}
