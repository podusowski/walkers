use crate::{
    InvalidZoom, Position,
    center::Center,
    position::AdjustedPosition,
    projector::{MercatorProjection, Projection},
    zoom::Zoom,
};
use egui::{DragPanButtons, Response, Vec2};

/// State of the map widget which must persist between frames, including its projection.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub struct MapMemory<P: Projection = MercatorProjection> {
    projection: P,
    center_mode: Center,
    zoom: Zoom,
}

impl<P: Projection> MapMemory<P> {
    /// Create map state using the given projection.
    pub fn new(projection: P) -> Self {
        Self {
            projection,
            center_mode: Center::default(),
            zoom: Zoom::default(),
        }
    }

    /// The projection used by this map state.
    pub fn projection(&self) -> &P {
        &self.projection
    }

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

    /// If the map is in detached state, returns the world position
    /// of the center. `None` if the map is not detached, i.e. following
    /// `my_position`.
    pub fn detached(&self) -> Option<Position> {
        self.center_mode.detached(&self.projection)
    }

    /// Whether the map is currently animating. Dragging, zooming and `my_position` changes are not
    /// considered animation.
    pub fn animating(&self) -> bool {
        self.center_mode.animating()
    }

    /// Point the map exactly at the given world position.
    pub fn center_at(&mut self, position: Position) {
        self.center_mode = Center::Exact(AdjustedPosition::new(position));
    }

    /// Start following `my_position` given in [`crate::Map::new`].
    pub fn follow_my_position(&mut self) {
        self.center_mode = Center::MyPosition;
    }

    pub(crate) fn position(&self, my_position: Position) -> Position {
        self.center_mode.position(my_position, &self.projection)
    }

    pub(crate) fn update_movement(&mut self, delta_time: f32) -> bool {
        self.center_mode
            .update_movement(delta_time, self.zoom.into())
    }

    pub(crate) fn handle_gestures(
        &mut self,
        response: &Response,
        my_position: Position,
        pull_to_my_position_threshold: f32,
        drag_pan_buttons: DragPanButtons,
    ) -> bool {
        self.center_mode.handle_gestures(
            response,
            my_position,
            pull_to_my_position_threshold,
            drag_pan_buttons,
        )
    }

    pub(crate) fn center_at_with_offset(&mut self, position: Position, offset: Vec2) {
        self.center_mode =
            Center::Exact(AdjustedPosition::new(position).shift(offset, self.zoom()));
    }

    pub(crate) fn shift_center(&mut self, offset: Vec2) {
        self.center_mode = self.center_mode.clone().shift(offset, self.zoom.into());
    }

    pub(crate) fn zoom_by(&mut self, value: f64) {
        self.zoom.zoom_by(value);
    }
}

impl Default for MapMemory<MercatorProjection> {
    fn default() -> Self {
        Self::new(MercatorProjection)
    }
}
