use egui::{Rect, Vec2};

use crate::{
    MapMemory, Position, mercator,
    position::{Pixels, PixelsExt as _},
};

/// Raw coordinate projection between world coordinates and pixel space.
///
/// Implementors define how a coordinate system maps to pixel coordinates at a
/// given zoom level. For GPS coordinates, use [`MercatorProjection`].
/// For pre-projected coordinates, use [`ProjectedProjection`].
pub trait Projection {
    /// Convert world coordinates to pixel coordinates at a given zoom level.
    fn position_to_pixels(&self, position: Position, zoom: f64) -> Pixels;

    /// Convert pixel coordinates back to world coordinates at a given zoom level.
    fn pixels_to_position(&self, pixels: Pixels, zoom: f64) -> Position;

    /// Scale factor: how many pixels correspond to one meter at this position and zoom level.
    fn scale_pixel_per_meter(&self, position: Position, zoom: f64) -> f32;
}

/// Web Mercator projection for GPS (lat/lon) coordinates.
pub struct MercatorProjection;

impl Projection for MercatorProjection {
    fn position_to_pixels(&self, position: Position, zoom: f64) -> Pixels {
        mercator::project(position, zoom)
    }

    fn pixels_to_position(&self, pixels: Pixels, zoom: f64) -> Position {
        mercator::unproject(pixels, zoom)
    }

    fn scale_pixel_per_meter(&self, position: Position, zoom: f64) -> f32 {
        const EARTH_CIRCUMFERENCE: f64 = 40_075_016.686;
        let total_pixels = mercator::total_pixels(zoom);
        let pixel_per_meter_equator = total_pixels / EARTH_CIRCUMFERENCE;
        let latitude_rad = position.y().abs().to_radians();
        (pixel_per_meter_equator / latitude_rad.cos()) as f32
    }
}

/// Linear projection for pre-projected coordinates (e.g., meters).
///
/// Positions are treated as (x, y) coordinates in a projected system.
/// The y-axis is flipped for screen rendering (positive y goes up in world space,
/// down in screen space).
pub struct ProjectedProjection;

impl Projection for ProjectedProjection {
    fn position_to_pixels(&self, position: Position, zoom: f64) -> Pixels {
        let scale = 2f64.powf(zoom);
        Pixels::new(position.x() * scale, -position.y() * scale)
    }

    fn pixels_to_position(&self, pixels: Pixels, zoom: f64) -> Position {
        let scale = 2f64.powf(zoom);
        Position::new(pixels.x() / scale, -pixels.y() / scale)
    }

    fn scale_pixel_per_meter(&self, _position: Position, zoom: f64) -> f32 {
        // For projected coordinates assumed to be in meters, scale is uniform.
        2f32.powf(zoom as f32)
    }
}

/// Screen projector that wraps a [`Projection`] with viewport state.
///
/// This is the standard [`Projector`] implementation used by the map widget.
/// It combines a raw [`Projection`] with the current clip rectangle and map memory
/// to convert between world coordinates and screen pixels.
#[derive(Clone)]
pub struct ScreenProjector<'a, P: Projection + ?Sized = dyn Projection> {
    projection: &'a P,
    clip_rect: Rect,
    memory: MapMemory,
    center_projected: Pixels,
}

impl<'a, P: Projection + ?Sized> ScreenProjector<'a, P> {
    pub fn new(
        projection: &'a P,
        clip_rect: Rect,
        map_memory: &MapMemory,
        my_position: Position,
    ) -> Self {
        let center = map_memory.center_mode.position(my_position, projection);
        let center_projected = projection.position_to_pixels(center, map_memory.zoom());
        Self {
            projection,
            clip_rect,
            memory: map_memory.to_owned(),
            center_projected,
        }
    }

    pub fn project(&self, position: Position) -> Vec2 {
        let projected = self
            .projection
            .position_to_pixels(position, self.memory.zoom());
        self.clip_rect.center().to_vec2() + (projected - self.center_projected).to_vec2()
    }

    pub fn unproject(&self, screen_position: Vec2) -> Position {
        let zoom = self.memory.zoom();
        let x = self.center_projected.x() + (screen_position.x as f64)
            - (self.clip_rect.center().x as f64);
        let y = self.center_projected.y() + (screen_position.y as f64)
            - (self.clip_rect.center().y as f64);
        self.projection.pixels_to_position(Pixels::new(x, y), zoom)
    }

    pub fn scale_pixel_per_meter(&self, position: Position) -> f32 {
        self.projection
            .scale_pixel_per_meter(position, self.memory.zoom())
    }
}

#[allow(clippy::unwrap_used)]
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

        let projector = ScreenProjector::new(
            &MercatorProjection,
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
    fn test_mercator_scale_at_equator() {
        let equator = lon_lat(0., 0.);
        let scale = MercatorProjection.scale_pixel_per_meter(equator, 0.);
        assert_approx_eq(scale as f64, 1. / 156_543.03);
    }

    #[test]
    fn unproject_is_inverse_of_project() {
        let original = lon_lat(21., 52.);

        let mut map_memory = MapMemory::default();
        map_memory.set_zoom(10.).unwrap();

        let projector = ScreenProjector::new(
            &MercatorProjection,
            Rect::from_min_size(Pos2::ZERO, Vec2::splat(100.)),
            &map_memory,
            original,
        );

        let projected = projector.project(original);
        let unprojected = projector.unproject(projected);

        assert_approx_eq(original.x(), unprojected.x());
        assert_approx_eq(original.y(), unprojected.y());
    }

    #[test]
    fn projected_roundtrip() {
        let original = Position::new(100.0, 200.0);

        let mut map_memory = MapMemory::default();
        map_memory.set_zoom(10.).unwrap();

        let projector = ScreenProjector::new(
            &ProjectedProjection,
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
