use egui::{PointerButton, Rect, Response, Sense, Ui, UiBuilder, Vec2, Widget};

use crate::{
    center::Center,
    mercator::{project, unproject},
    position::{AdjustedPosition, Pixels, PixelsExt},
    tiles::draw_tiles,
    zoom::{InvalidZoom, Zoom},
    Position, Projector, Tiles,
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

struct Layer<'a> {
    tiles: &'a mut dyn Tiles,
    transparency: f32,
}

/// The actual map widget. Instances are to be created on each frame, as all necessary state is
/// stored in [`Tiles`] and [`MapMemory`].
///
/// # Examples
///
/// ```
/// # use walkers::{Map, Tiles, MapMemory, Position, lon_lat};
///
/// fn update(ui: &mut egui::Ui, tiles: &mut dyn Tiles, map_memory: &mut MapMemory) {
///     ui.add(Map::new(
///         Some(tiles), // `None`, if you don't want to show any tiles.
///         map_memory,
///         lon_lat(17.03664, 51.09916)
///     ));
/// }
/// ```
pub struct Map<'a, 'b, 'c> {
    tiles: Option<&'b mut dyn Tiles>,
    layers: Vec<Layer<'b>>,
    memory: &'a mut MapMemory,
    my_position: Position,
    plugins: Vec<Box<dyn Plugin + 'c>>,

    zoom_gesture_enabled: bool,
    drag_gesture_enabled: bool,
    zoom_speed: f64,
    double_click_to_zoom: bool,
    double_click_to_zoom_out: bool,
    zoom_with_ctrl: bool,
    panning: bool,
}

impl<'a, 'b, 'c> Map<'a, 'b, 'c> {
    pub fn new(
        tiles: Option<&'b mut dyn Tiles>,
        memory: &'a mut MapMemory,
        my_position: Position,
    ) -> Self {
        Self {
            tiles,
            layers: Vec::default(),
            memory,
            my_position,
            plugins: Vec::default(),
            zoom_gesture_enabled: true,
            drag_gesture_enabled: true,
            zoom_speed: 2.0,
            double_click_to_zoom: false,
            double_click_to_zoom_out: false,
            zoom_with_ctrl: true,
            panning: true,
        }
    }

    /// Add plugin to the drawing pipeline. Plugins allow drawing custom shapes on the map.
    pub fn with_plugin(mut self, plugin: impl Plugin + 'c) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    /// Add a tile layer. All layers are drawn on top of each other with given transparency.
    pub fn with_layer(mut self, tiles: &'b mut dyn Tiles, transparency: f32) -> Self {
        self.layers.push(Layer {
            tiles,
            transparency,
        });
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

    /// Set if we can pan with mouse wheel.
    /// By default, panning is disabled when zooming with ctrl is disabled.
    /// Allow to disable panning even when zooming with ctrl is enabled.
    pub fn panning(mut self, enabled: bool) -> Self {
        self.panning = enabled;
        self
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
            // These values seem to correspond to the same values as one would get in `zoom_delta()`
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
            let offset = input_offset(ui, response);

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
        let panning_enabled =
            self.panning && (ui.input(|i| i.any_touches()) || self.zoom_with_ctrl);

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
        let delta_time = ui.ctx().input(|reader| reader.stable_dt);
        moved |= self.memory.center_mode.update_movement(delta_time);

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
            draw_tiles(&painter, map_center, zoom, tiles, 1.0);
        }

        for layer in self.layers {
            draw_tiles(&painter, map_center, zoom, layer.tiles, layer.transparency);
        }

        let projector = Projector::new(response.rect, self.memory, self.my_position);
        for (idx, plugin) in self.plugins.into_iter().enumerate() {
            let mut child_ui = ui.new_child(UiBuilder::new().max_rect(rect).id_salt(idx));
            plugin.run(&mut child_ui, &response, &projector);
        }

        response
    }
}

/// State of the map widget which must persist between frames.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub struct MapMemory {
    pub(crate) center_mode: Center,
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

/// Get the offset of the input (either mouse or touch) relative to the center.
fn input_offset(ui: &mut Ui, response: &Response) -> Option<Vec2> {
    let mouse_offset = response.hover_pos();
    let touch_offset = ui
        .input(|input| input.multi_touch())
        .map(|multi_touch| multi_touch.center_pos);

    // On touch we get both, so make touch the priority.
    touch_offset
        .or(mouse_offset)
        .map(|pos| pos - response.rect.center())
}
