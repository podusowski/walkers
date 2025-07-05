use egui::{Rect, Vec2};

use crate::{
    mercator::{project, unproject},
    position::{Pixels, PixelsExt as _},
    MapMemory, Position,
};

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
        let projected_position = project(position, self.memory.zoom());

        // We also need to know where the map center is.
        let map_center_projected_position = project(
            self.memory.center_mode.position(self.my_position),
            self.memory.zoom(),
        );

        // From the two points above we can calculate the actual point on the screen.
        self.clip_rect.center().to_vec2()
            + (projected_position - map_center_projected_position).to_vec2()
    }

    /// Get coordinates from viewport's pixels position
    pub fn unproject(&self, position: Vec2) -> Position {
        let zoom: f64 = self.memory.zoom();
        let center = self.memory.center_mode.position(self.my_position);

        // Despite being in pixel space `map_center_projected_position` is sufficiently large
        // that we must do the arithmetic in f64 to avoid imprecision.
        let map_center_projected_position = project(center, zoom);
        let clip_center = self.clip_rect.center();
        let x = map_center_projected_position.x() + (position.x as f64) - (clip_center.x as f64);
        let y = map_center_projected_position.y() + (position.y as f64) - (clip_center.y as f64);

        unproject(Pixels::new(x, y), zoom)
    }

    /// What is the local scale of the map at the provided position and given the current zoom
    /// level?
    pub fn scale_pixel_per_meter(&self, position: Position) -> f32 {
        let zoom = self.memory.zoom();

        // return f32 for ergonomics, as the result is typically used for egui code
        calculate_meters_per_pixel(position.y(), zoom) as f32
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
    use crate::lon_lat;
    use egui::Pos2;

    fn assert_approx_eq(a: f64, b: f64) {
        let diff = (a - b).abs();
        let tolerance = 0.01;
        assert!(
            diff < tolerance,
            "Values differ by more than {tolerance}: {a} vs {b}"
        );
    }

    #[test]
    fn test_unproject_precision() {
        let original = lon_lat(21., 52.);

        let mut map_memory = MapMemory::default();
        map_memory.set_zoom(18.).unwrap();

        let projector = Projector::new(
            Rect::from_min_size(Pos2::ZERO, Vec2::splat(100.)),
            &map_memory,
            original,
        );

        let mut projected = projector.project(original);
        let mut prev_x = 0.0;
        for offset in 0..10 {
            projected.x += offset as f32;
            let unprojected = projector.unproject(projected);
            assert_ne!(
                prev_x,
                unprojected.x(),
                "Input was different but projection remained the same"
            );
            prev_x = unprojected.x();
        }
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

    #[test]
    fn unproject_is_inverse_of_project() {
        let original = lon_lat(21., 52.);

        let mut map_memory = MapMemory::default();
        map_memory.set_zoom(10.).unwrap();

        let projector = Projector::new(
            Rect::from_min_size(Pos2::ZERO, Vec2::splat(100.)),
            &map_memory,
            original,
        );

        let projected = projector.project(original);
        let unprojected = projector.unproject(projected);

        assert_approx_eq(original.x(), unprojected.x());
        assert_approx_eq(original.y(), unprojected.y());
    }
}
