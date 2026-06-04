use egui::DragPanButtons;

pub struct Options {
    pub zoom_gesture_enabled: bool,
    pub drag_pan_buttons: DragPanButtons,
    pub zoom_speed: f64,
    pub double_click_to_zoom: bool,
    pub double_click_to_zoom_out: bool,
    pub zoom_with_ctrl: bool,
    pub panning: bool,
    pub pull_to_my_position_threshold: f32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            zoom_gesture_enabled: true,
            drag_pan_buttons: DragPanButtons::PRIMARY,
            zoom_speed: 2.0,
            double_click_to_zoom: false,
            double_click_to_zoom_out: false,
            zoom_with_ctrl: true,
            panning: true,
            pull_to_my_position_threshold: 0.0,
        }
    }
}
