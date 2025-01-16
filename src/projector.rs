use crate::{
    map_memory::MapMemory,
    units::{AdjustedPosition, Pixel, Position},
};
use egui::Rect;

/// Relationship between position and pixels
pub trait Projector {
    fn new(clip_rect: Rect, map_memory: &MapMemory, my_position: Position) -> Self;
    fn scale_pixel_per_meter(&self, position: Position) -> f32;
    fn project(&self, position: Position) -> Pixel;
    fn unproject(&self, pixel: Pixel) -> Position;
    fn position(&self, adjusted_pos: AdjustedPosition) -> Position {
        self.unproject(self.project(adjusted_pos.position) - adjusted_pos.offset)
    }
    fn zero_offset(&self, adjusted_pos: AdjustedPosition) -> AdjustedPosition {
        AdjustedPosition {
            position: self.position(adjusted_pos),
            offset: Default::default(),
        }
    }

    fn to_screen_coords(&self, pos: Pixel) -> egui::Vec2;
    fn from_screen_coords(&self, screen_pos: egui::Vec2) -> Pixel;
}

#[derive(Clone)]
pub struct LocalProjector {
    pub(crate) clip_rect: Rect,
    pub(crate) memory: MapMemory,
    pub(crate) my_position: Position,
}

impl LocalProjector {
    fn units_per_point(zoom: f64) -> f64 {
        0.001 * 2_f64.powf(20. - zoom)
    }
}

impl Projector for LocalProjector {
    fn new(clip_rect: Rect, map_memory: &MapMemory, my_position: Position) -> Self {
        Self {
            clip_rect,
            memory: map_memory.to_owned(),
            my_position,
        }
    }

    fn scale_pixel_per_meter(&self, _position: Position) -> f32 {
        Self::units_per_point(self.memory.zoom()) as f32
    }

    fn project(&self, position: Position) -> Pixel {
        let zoom = self.memory.zoom();
        let units_per_point = Self::units_per_point(zoom);

        Pixel::new(
            position.x() / units_per_point,
            position.y() / units_per_point,
        )
    }

    fn unproject(&self, position: Pixel) -> Position {
        // local pixel units
        let zoom = self.memory.zoom();
        let units_per_point = Self::units_per_point(zoom);

        Position::new(
            position.x() * units_per_point,
            position.y() * units_per_point,
        )
    }

    /// projects local coords into screen coords
    fn to_screen_coords(&self, pos: Pixel) -> egui::Vec2 {
        let map_center_projected_position =
            self.project(self.memory.center_mode.position(self.my_position, self));

        // From the two points above we can calculate the actual point on the screen.
        self.clip_rect.center().to_vec2() + (pos - map_center_projected_position).into()
    }

    /// projects local coords into flat mercator projection
    fn from_screen_coords(&self, pos: egui::Vec2) -> Pixel {
        let map_center_projected_position =
            self.project(self.memory.center_mode.position(self.my_position, self));

        map_center_projected_position + (pos - self.clip_rect.center().to_vec2()).into()
    }
}

// zoom level   tile coverage  number of tiles  tile size(*) in degrees
// 0            1 tile         1 tile           360° x 170.1022°
// 1            2 × 2 tiles    4 tiles          180° x 85.0511°
// 2            4 × 4 tiles    16 tiles         90° x [variable]

use std::f64::consts::PI;

/// Zoom specifies how many pixels are in the whole map. For example, zoom 0 means that the whole
/// map is just one 256x256 tile, zoom 1 means that it is 2x2 tiles, and so on.
pub(crate) fn total_pixels(zoom: f64) -> f64 {
    2f64.powf(zoom) * (TILE_SIZE as f64)
}

/// Size of a single tile in pixels. Walkers uses 256px tiles as most of the tile sources do.
const TILE_SIZE: u32 = 256;

/// Project the lat/lon coordinates into a 2D x/y using the Web Mercator.
/// <https://en.wikipedia.org/wiki/Web_Mercator_projection>
/// <https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames>
/// <https://www.netzwolf.info/osm/tilebrowser.html?lat=51.157800&lon=6.865500&zoom=14>
#[derive(Clone)]
pub struct GlobalProjector {
    pub(crate) clip_rect: Rect,
    pub(crate) memory: MapMemory,
    pub(crate) my_position: Position,
}

impl GlobalProjector {
    pub fn mercator_normalized(pos: Position) -> (f64, f64) {
        // Project into Mercator (cylindrical map projection).
        let x = pos.lon().to_radians();
        let y = pos.lat().to_radians().tan().asinh();

        // Scale both x and y to 0-1 range.
        let x = (1. + (x / PI)) / 2.;
        let y = (1. - (y / PI)) / 2.;
        (x, y)
    }
}

impl Projector for GlobalProjector {
    fn new(clip_rect: Rect, map_memory: &MapMemory, my_position: Position) -> Self {
        Self {
            clip_rect,
            memory: map_memory.to_owned(),
            my_position,
        }
    }

    /// What is the scale of the map at the provided position and
    /// given the current zoom level?
    fn scale_pixel_per_meter(&self, position: Position) -> f32 {
        const EARTH_CIRCUMFERENCE: f64 = 40_075_016.686;

        // Number of pixels for width of world at this zoom level
        let total_pixels = total_pixels(self.memory.zoom());

        let pixel_per_meter_equator = total_pixels / EARTH_CIRCUMFERENCE;
        let latitude_rad = position.lat().abs().to_radians();
        (pixel_per_meter_equator / latitude_rad.cos()) as f32
    }

    /// projects lat lon into a flat mercator projection
    fn project(&self, position: Position) -> Pixel {
        let zoom = self.memory.zoom();

        let total_pixels = total_pixels(zoom);

        // Turn that into a flat, mercator projection.
        let (x, y) = Self::mercator_normalized(position);

        Pixel::new(x * total_pixels, y * total_pixels)
    }

    /// unprojects flat mercator into lat lon
    fn unproject(&self, screen_pos: Pixel) -> Position {
        // for pixel
        let number_of_pixels: f64 = 2f64.powf(self.memory.zoom()) * (TILE_SIZE as f64);

        let lon = screen_pos.x();
        let lon = lon / number_of_pixels;
        let lon = (lon * 2. - 1.) * PI;
        let lon = lon.to_degrees();

        let lat = screen_pos.y();
        let lat = lat / number_of_pixels;
        let lat = (-lat * 2. + 1.) * PI;
        let lat = lat.sinh().atan().to_degrees();

        Position::from_lon_lat(lon, lat)
    }

    /// projects flat mercator projection into screen coords
    fn to_screen_coords(&self, pos: Pixel) -> egui::Vec2 {
        let map_center_projected_position =
            self.project(self.memory.center_mode.position(self.my_position, self));

        // From the two points above we can calculate the actual point on the screen.
        self.clip_rect.center().to_vec2() + (pos - map_center_projected_position).into()
    }

    /// projects screen coords into flat mercator projection
    fn from_screen_coords(&self, pos: egui::Vec2) -> Pixel {
        let map_center_projected_position =
            self.project(self.memory.center_mode.position(self.my_position, self));

        map_center_projected_position + (pos - self.clip_rect.center().to_vec2()).into()
    }
}
