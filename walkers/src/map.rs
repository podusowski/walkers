use std::collections::{hash_map::Entry, HashMap};

use egui::{Mesh, PointerButton, Rect, Response, Sense, Ui, UiBuilder, Vec2, Widget};

use crate::{
    center::Center,
    mercator::{screen_to_position, Pixels, PixelsExt, TileId},
    tiles,
    zoom::{InvalidZoom, Zoom},
    Position, Tiles,
};

/// Plugins allow drawing custom shapes on the map. After implementing this trait for your type,
/// you can add it to the map with [`Map::with_plugin`]
pub trait Plugin {
    /// Function called at each frame.
    ///
    /// The provided [`Ui`] has its [`Ui::max_rect`] set to the full rect that was allocated
    /// by the map widget. Implementations should typically use the provided [`Projector`] to
    /// compute target screen coordinates and use one of the various egui methods to draw at these
    /// coordinates instead of relying on [`Ui`] layout system.
    ///
    /// The provided [`Response`] is the response of the map widget itself and can be used to test
    /// if the mouse is hovering or clicking on the map.
    fn run(self: Box<Self>, ui: &mut Ui, response: &Response, projector: &Projector);
}

/// The actual map widget. Instances are to be created on each frame, as all necessary state is
/// stored in [`Tiles`] and [`MapMemory`].
///
/// # Examples
///
/// ```
/// # use walkers::{Map, Tiles, MapMemory, Position};
///
/// fn update(ui: &mut egui::Ui, tiles: &mut dyn Tiles, map_memory: &mut MapMemory) {
///     ui.add(Map::new(
///         Some(tiles), // `None`, if you don't want to show any tiles.
///         map_memory,
///         Position::from_lon_lat(17.03664, 51.09916)
///     ));
/// }
/// ```
pub struct Map<'a, 'b, 'c> {
    tiles: Option<&'b mut dyn Tiles>,
    memory: &'a mut MapMemory,
    my_position: Position,
    plugins: Vec<Box<dyn Plugin + 'c>>,

    zoom_gesture_enabled: bool,
    drag_gesture_enabled: bool,
    zoom_speed: f64,
    double_click_to_zoom: bool,
    double_click_to_zoom_out: bool,
    zoom_with_ctrl: bool,
}

impl<'a, 'b, 'c> Map<'a, 'b, 'c> {
    pub fn new(
        tiles: Option<&'b mut dyn Tiles>,
        memory: &'a mut MapMemory,
        my_position: Position,
    ) -> Self {
        Self {
            tiles,
            memory,
            my_position,
            plugins: Vec::default(),
            zoom_gesture_enabled: true,
            drag_gesture_enabled: true,
            zoom_speed: 2.0,
            double_click_to_zoom: false,
            double_click_to_zoom_out: false,
            zoom_with_ctrl: true,
        }
    }

    /// Add plugin to the drawing pipeline. Plugins allow drawing custom shapes on the map.
    pub fn with_plugin(mut self, plugin: impl Plugin + 'c) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    /// Set whether map should perform zoom gesture.
    ///
    /// Zoom is typically triggered by the mouse wheel while holding <kbd>ctrl</kbd> key on native
    /// and web, and by pinch gesture on Android.
    pub fn zoom_gesture(mut self, enabled: bool) -> Self {
        self.zoom_gesture_enabled = enabled;
        self
    }

    /// Set whether map should perform drag gesture.
    pub fn drag_gesture(mut self, enabled: bool) -> Self {
        self.drag_gesture_enabled = enabled;
        self
    }

    /// Change how far to zoom in/out.
    /// Default value is 2.0
    pub fn zoom_speed(mut self, speed: f64) -> Self {
        self.zoom_speed = speed;
        self
    }

    /// Set whether to enable double click primary mouse button to zoom
    pub fn double_click_to_zoom(mut self, enabled: bool) -> Self {
        self.double_click_to_zoom = enabled;
        self
    }

    /// Set whether to enable double click secondary mouse button to zoom out
    pub fn double_click_to_zoom_out(mut self, enabled: bool) -> Self {
        self.double_click_to_zoom_out = enabled;
        self
    }

    /// Sets the zoom behaviour
    ///
    /// When enabled zoom is done with mouse wheel while holding <kbd>ctrl</kbd> key on native
    /// and web. Panning is done with mouse wheel without <kbd>ctrl</kbd> key
    ///
    /// When disabled, zooming can be done without holding <kbd>ctrl</kbd> key
    /// but panning with mouse wheel is disabled
    ///
    /// Has no effect on Android
    pub fn zoom_with_ctrl(mut self, enabled: bool) -> Self {
        self.zoom_with_ctrl = enabled;
        self
    }
}

/// Projects geographical position into pixels on the viewport, suitable for [`egui::Painter`].
#[derive(Clone)]
pub struct Projector {
    clip_rect: Rect,
    memory: MapMemory,
    my_position: Position,
}

impl Projector {
    pub fn new(clip_rect: Rect, map_memory: &MapMemory, my_position: Position) -> Self {
        Self {
            clip_rect,
            memory: map_memory.to_owned(),
            my_position,
        }
    }

    /// Project `position` into pixels on the viewport.
    pub fn project(&self, position: Position) -> Vec2 {
        // Turn that into a flat, mercator projection.
        let projected_position = position.project(self.memory.zoom.into());

        // We need the precision of f64 here,
        // since some "gaps" between tiles are noticeable on large zoom levels (e.g. 16+)
        let zoom: f64 = self.memory.zoom.into();

        // We also need to know where the map center is.
        let map_center_projected_position = self
            .memory
            .center_mode
            .position(self.my_position, zoom)
            .project(self.memory.zoom.into());

        // From the two points above we can calculate the actual point on the screen.
        self.clip_rect.center().to_vec2()
            + (projected_position - map_center_projected_position).to_vec2()
    }

    /// Get coordinates from viewport's pixels position
    pub fn unproject(&self, position: Vec2) -> Position {
        let zoom: f64 = self.memory.zoom.into();
        let center = self.memory.center_mode.position(self.my_position, zoom);

        AdjustedPosition {
            position: center,
            offset: Default::default(),
        }
        .shift(-position)
        .position(zoom)
    }

    /// What is the local scale of the map at the provided position and given the current zoom
    /// level?
    pub fn scale_pixel_per_meter(&self, position: Position) -> f32 {
        let zoom = self.memory.zoom.into();

        // return f32 for ergonomics, as the result is typically used for egui code
        calculate_meters_per_pixel(position.lat(), zoom) as f32
    }
}

impl Map<'_, '_, '_> {
    /// Handle zoom and drag inputs, and recalculate everything accordingly.
    /// Returns `false` if no gesture handled.
    fn handle_gestures(&mut self, ui: &mut Ui, response: &Response) -> bool {
        let mut zoom_delta = ui.input(|input| input.zoom_delta()) as f64;

        if self.double_click_to_zoom
            && ui.ui_contains_pointer()
            && response.double_clicked_by(PointerButton::Primary)
        {
            zoom_delta = 2.0;
        }

        if self.double_click_to_zoom_out
            && ui.ui_contains_pointer()
            && response.double_clicked_by(PointerButton::Secondary)
        {
            zoom_delta = 0.0;
        }

        if !self.zoom_with_ctrl && zoom_delta == 1.0 {
            // We only use the raw scroll values, if we are zooming without ctrl,
            // and zoom_delta is not already over/under 1.0 (eg. a ctrl + scroll event or a pinch zoom)
            // These values seem to corrospond to the same values as one would get in `zoom_delta()`
            zoom_delta = ui.input(|input| (1.0 + input.smooth_scroll_delta.y / 200.0)) as f64
        };

        let mut changed = false;

        // Zooming and dragging need to be exclusive, otherwise the map will get dragged when
        // pinch gesture is used.
        if !(0.99..=1.01).contains(&zoom_delta)
            && ui.ui_contains_pointer()
            && self.zoom_gesture_enabled
        {
            // Displacement of mouse pointer relative to widget center
            let offset = response.hover_pos().map(|p| p - response.rect.center());

            let pos = self
                .memory
                .center_mode
                .position(self.my_position, self.memory.zoom());

            // While zooming, we want to keep the location under the mouse pointer fixed on the
            // screen. To achieve this, we first move the location to the widget's center,
            // then adjust zoom level, finally move the location back to the original screen
            // position.
            if let Some(offset) = offset {
                self.memory.center_mode = Center::Exact(
                    AdjustedPosition::from(pos)
                        .shift(-offset)
                        .zero_offset(self.memory.zoom.into()),
                );
            }

            // Shift by 1 because of the values given by zoom_delta(). Multiple by zoom_speed(defaults to 2.0),
            // because then it felt right with both mouse wheel, and an Android phone.
            self.memory
                .zoom
                .zoom_by((zoom_delta - 1.) * self.zoom_speed);

            // Recalculate the AdjustedPosition's offset, since it gets invalidated by zooming.
            self.memory.center_mode = self
                .memory
                .center_mode
                .clone()
                .zero_offset(self.memory.zoom.into());

            if let Some(offset) = offset {
                self.memory.center_mode = self.memory.center_mode.clone().shift(offset);
            }

            changed = true;
        } else if self.drag_gesture_enabled {
            changed = self
                .memory
                .center_mode
                .recalculate_drag(response, self.my_position);
        }

        // Only enable panning with mouse_wheel if we are zooming with ctrl. But always allow touch devices to pan
        let panning_enabled = ui.input(|i| i.any_touches()) || self.zoom_with_ctrl;

        if ui.ui_contains_pointer() && panning_enabled {
            // Panning by scrolling, e.g. two-finger drag on a touchpad:
            let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
            if scroll_delta != Vec2::ZERO {
                let pos = self
                    .memory
                    .center_mode
                    .position(self.my_position, self.memory.zoom());
                self.memory.center_mode =
                    Center::Exact(AdjustedPosition::from(pos).shift(scroll_delta));
            }
        }

        changed
    }
}

impl Widget for Map<'_, '_, '_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let (rect, mut response) =
            ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());

        let mut moved = self.handle_gestures(ui, &response);
        moved |= self.memory.center_mode.update_movement();

        if moved {
            response.mark_changed();
            ui.ctx().request_repaint();
        }

        let zoom = self.memory.zoom;
        let map_center = self
            .memory
            .center_mode
            .position(self.my_position, zoom.into());
        let painter = ui.painter().with_clip_rect(rect);

        if let Some(tiles) = self.tiles {
            let mut meshes = Default::default();
            flood_fill_tiles(
                painter.clip_rect(),
                map_center.tile_id(zoom.round(), tiles.tile_size()),
                map_center.project(zoom.into()),
                zoom.into(),
                tiles,
                &mut meshes,
            );

            for shape in meshes.drain().filter_map(|(_, mesh)| mesh) {
                painter.add(shape);
            }
        }

        let projector = Projector::new(response.rect, self.memory, self.my_position);
        for (idx, plugin) in self.plugins.into_iter().enumerate() {
            let mut child_ui = ui.new_child(UiBuilder::new().max_rect(rect).id_salt(idx));
            plugin.run(&mut child_ui, &response, &projector);
        }

        response
    }
}

/// [`Position`] alone is not able to represent detached (e.g. after map gets dragged) position
/// due to insufficient accuracy.
#[derive(Debug, Clone, PartialEq)]
pub struct AdjustedPosition {
    /// Base geographical position.
    pub position: Position,

    /// Offset in pixels.
    pub offset: Pixels,
}

impl AdjustedPosition {
    pub(crate) fn new(position: Position, offset: Pixels) -> Self {
        Self { position, offset }
    }

    /// Calculate the real position, i.e. including the offset.
    pub(crate) fn position(&self, zoom: f64) -> Position {
        screen_to_position(self.position.project(zoom) - self.offset, zoom)
    }

    /// Recalculate `position` so that `offset` is zero.
    pub(crate) fn zero_offset(self, zoom: f64) -> Self {
        Self {
            position: screen_to_position(self.position.project(zoom) - self.offset, zoom),
            offset: Default::default(),
        }
    }

    pub(crate) fn shift(self, offset: Vec2) -> Self {
        Self {
            position: self.position,
            offset: self.offset + Pixels::new(offset.x as f64, offset.y as f64),
        }
    }
}

impl From<Position> for AdjustedPosition {
    fn from(position: Position) -> Self {
        Self {
            position,
            offset: Default::default(),
        }
    }
}

/// State of the map widget which must persist between frames.
#[derive(Debug, Default, Clone)]
pub struct MapMemory {
    center_mode: Center,
    zoom: Zoom,
}

impl MapMemory {
    /// Try to zoom in, returning `Err(InvalidZoom)` if already at maximum.
    pub fn zoom_in(&mut self) -> Result<(), InvalidZoom> {
        self.center_mode = self.center_mode.clone().zero_offset(self.zoom.into());
        self.zoom.zoom_in()
    }

    /// Try to zoom out, returning `Err(InvalidZoom)` if already at minimum.
    pub fn zoom_out(&mut self) -> Result<(), InvalidZoom> {
        self.center_mode = self.center_mode.clone().zero_offset(self.zoom.into());
        self.zoom.zoom_out()
    }

    /// Set exact zoom level
    pub fn set_zoom(&mut self, zoom: f64) -> Result<(), InvalidZoom> {
        self.center_mode = self.center_mode.clone().zero_offset(self.zoom.into());
        self.zoom = Zoom::try_from(zoom)?;
        Ok(())
    }

    /// Returns the current zoom level
    pub fn zoom(&self) -> f64 {
        self.zoom.into()
    }

    /// Returns exact position if map is detached (i.e. not following `my_position`),
    /// `None` otherwise.
    pub fn detached(&self) -> Option<Position> {
        self.center_mode.detached(self.zoom.into())
    }

    /// Center exactly at the given position.
    pub fn center_at(&mut self, position: Position) {
        self.center_mode = Center::Exact(AdjustedPosition {
            position,
            offset: Default::default(),
        });
    }

    /// Follow `my_position`.
    pub fn follow_my_position(&mut self) {
        self.center_mode = Center::MyPosition;
    }
}

/// Use simple [flood fill algorithm](https://en.wikipedia.org/wiki/Flood_fill) to draw tiles on the map.
fn flood_fill_tiles(
    viewport: Rect,
    tile_id: TileId,
    map_center_projected_position: Pixels,
    zoom: f64,
    tiles: &mut dyn Tiles,
    meshes: &mut HashMap<TileId, Option<Mesh>>,
) {
    // We need to make up the difference between integer and floating point zoom levels.
    let corrected_tile_size = tiles.tile_size() as f64 * 2f64.powf(zoom - zoom.round());
    let tile_projected = tile_id.project(corrected_tile_size);
    let tile_screen_position =
        viewport.center().to_vec2() + (tile_projected - map_center_projected_position).to_vec2();

    if viewport.intersects(tiles::rect(tile_screen_position, corrected_tile_size)) {
        if let Entry::Vacant(entry) = meshes.entry(tile_id) {
            // It's still OK to insert an empty one, as we need to mark the spot for the filling algorithm.
            let tile = tiles.at(tile_id).map(|tile| {
                tile.texture
                    .mesh_with_uv(tile_screen_position, corrected_tile_size, tile.uv)
            });

            entry.insert(tile);

            for next_tile_id in [
                tile_id.north(),
                tile_id.east(),
                tile_id.south(),
                tile_id.west(),
            ]
            .iter()
            .flatten()
            {
                flood_fill_tiles(
                    viewport,
                    *next_tile_id,
                    map_center_projected_position,
                    zoom,
                    tiles,
                    meshes,
                );
            }
        }
    }
}

/// Implementation of the scale computation.
fn calculate_meters_per_pixel(latitude: f64, zoom: f64) -> f64 {
    const EARTH_CIRCUMFERENCE: f64 = 40_075_016.686;

    // Number of pixels for width of world at this zoom level
    let total_pixels = crate::mercator::total_pixels(zoom);

    let pixel_per_meter_equator = total_pixels / EARTH_CIRCUMFERENCE;
    let latitude_rad = latitude.abs().to_radians();
    pixel_per_meter_equator / latitude_rad.cos()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_approx_eq(a: f64, b: f64) {
        let diff = (a - b).abs();
        let tolerance = 0.01;
        assert!(
            diff < tolerance,
            "Values differ by more than {tolerance}: {a} vs {b}"
        );
    }

    #[test]
    fn test_equator_zoom_0() {
        // At zoom 0 (whole world), equator should be about 156.5km per pixel
        let scale = calculate_meters_per_pixel(0.0, 0.);
        assert_approx_eq(scale, 1. / 156_543.03);
    }

    #[test]
    fn test_equator_zoom_19() {
        // At max zoom (19), equator should be about 0.3m per pixel
        let scale = calculate_meters_per_pixel(0.0, 19.);
        assert_approx_eq(scale, 1. / 0.298);
    }
}
