use egui::{Rect, Vec2};

use crate::{
    MapMemory, Position, Projection,
    position::{Pixels, PixelsExt as _},
};

/// Projects geographical position into pixels on the viewport, suitable for [`egui::Painter`].
#[derive(Clone)]
pub struct Projector<'a> {
    projection: &'a dyn Projection,
    clip_rect: Rect,
    zoom: f64,
    map_center_projected_position: Pixels,
}

impl<'a> Projector<'a> {
    pub fn new(
        clip_rect: Rect,
        projection: &'a dyn Projection,
        map_memory: &MapMemory,
        my_position: Position,
    ) -> Self {
        let zoom = map_memory.zoom();
        let map_center_projected_position =
            projection.project(map_memory.center_mode.position(my_position), zoom);

        Self {
            projection,
            clip_rect,
            zoom,
            map_center_projected_position,
        }
    }

    /// Projection used by the map.
    pub fn projection(&self) -> &'a dyn Projection {
        self.projection
    }

    /// Project `position` into pixels on the viewport.
    pub fn project(&self, position: Position) -> Vec2 {
        let projected_position = self.projection.project(position, self.zoom);

        // From the two points above we can calculate the actual point on the screen.
        self.clip_rect.center().to_vec2()
            + (projected_position - self.map_center_projected_position).to_vec2()
    }

    /// Get coordinates from viewport's pixels position
    pub fn unproject(&self, position: Vec2) -> Position {
        // Despite being in pixel space `map_center_projected_position` is sufficiently large
        // that we must do the arithmetic in f64 to avoid imprecision.
        let clip_center = self.clip_rect.center();
        let x =
            self.map_center_projected_position.x() + (position.x as f64) - (clip_center.x as f64);
        let y =
            self.map_center_projected_position.y() + (position.y as f64) - (clip_center.y as f64);

        self.projection.unproject(Pixels::new(x, y), self.zoom)
    }

    /// What is the local scale of the map at the provided position and given the current zoom
    /// level?
    pub fn scale_pixel_per_meter(&self, position: Position) -> f32 {
        // return f32 for ergonomics, as the result is typically used for egui code
        self.projection.scale_pixel_per_meter(position, self.zoom) as f32
    }

    pub(crate) fn map_center_projected_position(&self) -> Pixels {
        self.map_center_projected_position
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MercatorProjection, lon_lat};
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
            &MercatorProjection,
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
    fn unproject_is_inverse_of_project() {
        let original = lon_lat(21., 52.);

        let mut map_memory = MapMemory::default();
        map_memory.set_zoom(10.).unwrap();

        let projector = Projector::new(
            Rect::from_min_size(Pos2::ZERO, Vec2::splat(100.)),
            &MercatorProjection,
            &map_memory,
            original,
        );

        let projected = projector.project(original);
        let unprojected = projector.unproject(projected);

        assert_approx_eq(original.x(), unprojected.x());
        assert_approx_eq(original.y(), unprojected.y());
    }
}
