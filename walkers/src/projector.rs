use egui::{Pos2, Rect};

use crate::{
    MapMemory, Position, equal_earth, mercator,
    position::{Pixels, PixelsExt as _},
};

const EARTH_RADIUS_METERS: f64 = 6_378_137.0;

/// What kind of coordinates a projection expects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateKind {
    /// Longitude and latitude in degrees.
    Geographic,
    /// Cartesian coordinates in linear units.
    Projected,
}

/// Raw coordinate projection between world coordinates and pixel space.
///
/// Implementors define how a coordinate system maps to pixel coordinates at a given zoom level.
///
/// For geographic coordinates, use [`MercatorProjection`] or [`EqualEarthProjection`].
/// For cartesian coordinates, use [`PlanarProjection`].
///
/// Or implement your own for your custom tile server
pub trait Projection {
    /// Convert world coordinates to pixel coordinates at a given zoom level.
    fn position_to_pixels(&self, position: Position, zoom: f64) -> Pixels;

    /// Convert pixel coordinates back to world coordinates at a given zoom level.
    fn pixels_to_position(&self, pixels: Pixels, zoom: f64) -> Position;

    /// Nominal scale factor: how many pixels correspond to one meter at this position and zoom
    /// level. For non-conformal projections, implementations must document how this representative
    /// scalar is chosen.
    fn scale_pixel_per_meter(&self, position: Position, zoom: f64) -> f32;

    /// What coordinates the projection projects from
    fn coordinate_kind(&self) -> CoordinateKind;
}

/// Web Mercator projection for GPS (lat/lon) coordinates.
#[derive(Debug, Clone)]
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

    fn coordinate_kind(&self) -> CoordinateKind {
        CoordinateKind::Geographic
    }
}

/// Spherical Equal Earth projection for longitude/latitude coordinates.
///
/// Equal Earth is an equal-area pseudocylindrical projection intended for world maps.
/// The full equatorial width occupies 256 pixels at zoom level zero and doubles with each zoom level.
/// The shorter projected height is centered within the same square world-pixel space.
///
/// Unlike Web Mercator, Equal Earth is not conformal: local distances can have different horizontal and vertical scales.
/// [`Projection::scale_pixel_per_meter`] therefore returns the area-equivalent nominal linear scale.
#[derive(Debug, Clone)]
pub struct EqualEarthProjection;

impl Projection for EqualEarthProjection {
    fn position_to_pixels(&self, position: Position, zoom: f64) -> Pixels {
        let total_pixels = mercator::total_pixels(zoom);
        let scale = total_pixels / (2.0 * equal_earth::MAX_X);
        let projected = equal_earth::project(position);

        Pixels::new(
            total_pixels / 2.0 + projected.x() * scale,
            total_pixels / 2.0 - projected.y() * scale,
        )
    }

    fn pixels_to_position(&self, pixels: Pixels, zoom: f64) -> Position {
        let total_pixels = mercator::total_pixels(zoom);
        let scale = total_pixels / (2.0 * equal_earth::MAX_X);
        let projected = Pixels::new(
            (pixels.x() - total_pixels / 2.0) / scale,
            (total_pixels / 2.0 - pixels.y()) / scale,
        );

        equal_earth::unproject(projected)
    }

    fn scale_pixel_per_meter(&self, _position: Position, zoom: f64) -> f32 {
        let total_pixels = mercator::total_pixels(zoom);
        let projected_pixels_per_radian = total_pixels / (2.0 * equal_earth::MAX_X);
        (projected_pixels_per_radian / EARTH_RADIUS_METERS) as f32
    }

    fn coordinate_kind(&self) -> CoordinateKind {
        CoordinateKind::Geographic
    }
}

/// Maps positions from an already-projected, meter-based Cartesian
/// coordinate system into Walkers' zoomed pixel space.
///
/// `origin` maps to the pixel origin. At zoom level zero,
/// `pixels_per_meter` determines the uniform scale; each additional zoom
/// level doubles it. The y-axis is reversed so positive world y points
/// upward while positive screen y points downward.
#[derive(Debug, Clone)]
pub struct PlanarProjection {
    /// Origin of the projection in world coordinates.
    pub origin: Position,
    /// Pixels per meter at zoom level zero.
    pub pixels_per_meter_at_zoom_zero: f64,
}

impl PlanarProjection {
    pub fn new(origin: Position, pixels_per_meter_at_zoom_zero: f64) -> Self {
        Self {
            origin,
            pixels_per_meter_at_zoom_zero,
        }
    }
}

impl Projection for PlanarProjection {
    fn position_to_pixels(&self, position: Position, zoom: f64) -> Pixels {
        let scale = self.pixels_per_meter_at_zoom_zero * 2f64.powf(zoom);
        let dx = position.x() - self.origin.x();
        let dy = position.y() - self.origin.y();
        Pixels::new(dx * scale, -dy * scale)
    }

    fn pixels_to_position(&self, pixels: Pixels, zoom: f64) -> Position {
        let scale = self.pixels_per_meter_at_zoom_zero * 2f64.powf(zoom);
        Position::new(
            self.origin.x() + pixels.x() / scale,
            self.origin.y() - pixels.y() / scale,
        )
    }

    fn scale_pixel_per_meter(&self, _position: Position, zoom: f64) -> f32 {
        // For projected coordinates assumed to be in meters, scale is uniform.
        (self.pixels_per_meter_at_zoom_zero * 2f64.powf(zoom)) as f32
    }

    fn coordinate_kind(&self) -> CoordinateKind {
        CoordinateKind::Projected
    }
}

/// Screen projector that wraps a [`Projection`] with viewport state.
///
/// This is the standard projector implementation used by the map widget.
/// It combines a raw [`Projection`] with the current clip rectangle and map memory
/// to convert between world coordinates and screen pixels.
#[derive(Debug, Clone)]
pub struct ScreenProjector<'a, P: Projection + ?Sized = dyn Projection> {
    pub projection: &'a P,
    pub clip_rect: Rect,
    zoom: f64,
    pub(crate) center_projected: Pixels,
}

impl<'a, P: Projection + ?Sized> ScreenProjector<'a, P> {
    pub fn new(
        projection: &'a P,
        clip_rect: Rect,
        map_memory: &MapMemory,
        my_position: Position,
    ) -> Self {
        let center = map_memory.center_mode.position(my_position, projection);
        let zoom = map_memory.zoom();
        let center_projected = projection.position_to_pixels(center, zoom);
        Self {
            projection,
            clip_rect,
            zoom,
            center_projected,
        }
    }

    pub fn project(&self, position: Position) -> Pos2 {
        let projected = self.projection.position_to_pixels(position, self.zoom);
        (self.clip_rect.center().to_vec2() + (projected - self.center_projected).to_vec2())
            .to_pos2()
    }

    pub fn unproject(&self, screen_position: Pos2) -> Position {
        let x = self.center_projected.x() + (screen_position.x as f64)
            - (self.clip_rect.center().x as f64);
        let y = self.center_projected.y() + (screen_position.y as f64)
            - (self.clip_rect.center().y as f64);
        self.projection
            .pixels_to_position(Pixels::new(x, y), self.zoom)
    }

    pub fn scale_pixel_per_meter(&self, position: Position) -> f32 {
        self.projection.scale_pixel_per_meter(position, self.zoom)
    }

    pub fn zoom(&self) -> f64 {
        self.zoom
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lon_lat;
    use egui::{Pos2, Vec2};

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
    fn equal_earth_uses_the_zoom_zero_world_width() {
        let west = EqualEarthProjection.position_to_pixels(lon_lat(-180.0, 0.0), 0.0);
        let center = EqualEarthProjection.position_to_pixels(lon_lat(0.0, 0.0), 0.0);
        let east = EqualEarthProjection.position_to_pixels(lon_lat(180.0, 0.0), 0.0);

        assert_approx_eq(west.x(), 0.0);
        assert_approx_eq(center.x(), 128.0);
        assert_approx_eq(center.y(), 128.0);
        assert_approx_eq(east.x(), 256.0);
    }

    #[test]
    fn equal_earth_roundtrip() {
        let original = lon_lat(21.0, 52.0);
        let pixels = EqualEarthProjection.position_to_pixels(original, 10.0);
        let unprojected = EqualEarthProjection.pixels_to_position(pixels, 10.0);

        assert_approx_eq(unprojected.x(), original.x());
        assert_approx_eq(unprojected.y(), original.y());
    }

    #[test]
    fn equal_earth_scale_doubles_at_each_zoom_level() {
        let position = lon_lat(21.0, 52.0);
        let scale_at_zero = EqualEarthProjection.scale_pixel_per_meter(position, 0.0);
        let scale_at_one = EqualEarthProjection.scale_pixel_per_meter(position, 1.0);

        assert_approx_eq(scale_at_one.into(), (scale_at_zero * 2.0).into());
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

        let projection = PlanarProjection::new(original, 1.0);
        let projector = ScreenProjector::new(
            &projection,
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
