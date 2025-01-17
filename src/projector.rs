use crate::{
    map_memory::MapMemory,
    tiles::TileId,
    units::{AdjustedPosition, Pixel, Position},
};

/// A Projector relates Positions to screen coordinates
/// two projectors are supported.
pub enum Projector {
    /// Global is used for the regular map where Positions are latitude and longitude
    /// and are projected using mercator projection
    Global(GlobalProjector),
    /// Local is used for local coordinates were Positions are euclidean x and y values in
    /// some arbitrary units and the projection is an affine transformation
    Local(LocalProjector),
}

// distribute the function call to the correct projector version
impl Projector {
    /// get the local scale of the map at this position
    pub fn scale_pixel_per_meter(&self, position: Position) -> f32 {
        match self {
            Projector::Global(global_projector) => global_projector.scale_pixel_per_meter(position),
            Projector::Local(local_projector) => local_projector.scale_pixel_per_meter(position),
        }
    }

    /// project the position to screen coordinates
    pub fn project(&self, position: Position) -> egui::Pos2 {
        match self {
            Projector::Global(global_projector) => global_projector.project(position),
            Projector::Local(local_projector) => local_projector.project(position),
        }
    }

    /// unproject the screen coordinates to a position
    pub fn unproject(&self, pixel: egui::Pos2) -> Position {
        match self {
            Projector::Global(global_projector) => global_projector.unproject(pixel),
            Projector::Local(local_projector) => local_projector.unproject(pixel),
        }
    }

    pub(crate) fn set_clip_rect(&mut self, rect: egui::Rect) {
        match self {
            Projector::Global(global_projector) => global_projector.set_clip_rect(rect),
            Projector::Local(local_projector) => local_projector.set_clip_rect(rect),
        }
    }

    pub(crate) fn tile_id(&self, pos: Position, zoom: u8, tile_size: u32) -> Option<TileId> {
        match self {
            Projector::Global(global_projector) => global_projector.tile_id(pos, zoom, tile_size),
            Projector::Local(local_projector) => local_projector.tile_id(pos, zoom, tile_size),
        }
    }

    pub(crate) fn position(&self, adjusted_pos: AdjustedPosition) -> Position {
        match self {
            Projector::Global(global_projector) => global_projector.position(adjusted_pos),
            Projector::Local(local_projector) => local_projector.position(adjusted_pos),
        }
    }

    pub(crate) fn zero_offset(&self, adjusted_pos: AdjustedPosition) -> AdjustedPosition {
        match &self {
            Projector::Global(global_projector) => global_projector.zero_offset(adjusted_pos),
            Projector::Local(local_projector) => local_projector.zero_offset(adjusted_pos),
        }
    }

    fn to_screen_coords(&self, pixel: Pixel) -> egui::Pos2 {
        match self {
            Projector::Global(global_projector) => global_projector.to_screen_coords(pixel),
            Projector::Local(local_projector) => local_projector.to_screen_coords(pixel),
        }
    }

    fn from_screen_coords(&self, screen_pos: egui::Pos2) -> Pixel {
        match self {
            Projector::Global(global_projector) => global_projector.from_screen_coords(screen_pos),
            Projector::Local(local_projector) => local_projector.from_screen_coords(screen_pos),
        }
    }

    fn pixel_project(&self, pos: Position) -> Pixel {
        match self {
            Projector::Global(global_projector) => global_projector.pixel_project(pos),
            Projector::Local(local_projector) => local_projector.pixel_project(pos),
        }
    }

    fn pixel_unproject(&self, pixel: Pixel) -> Position {
        match self {
            Projector::Global(global_projector) => global_projector.pixel_unproject(pixel),
            Projector::Local(local_projector) => local_projector.pixel_unproject(pixel),
        }
    }
}

pub(crate) trait ProjectorTrait {
    // exposed through Projector enum
    // get the scale of the map a given position
    fn scale_pixel_per_meter(&self, position: Position) -> f32;
    fn project(&self, position: Position) -> egui::Pos2;
    fn unproject(&self, pixel: egui::Pos2) -> Position;

    // used within crate
    fn tile_id(&self, pos: Position, zoom: u8, tile_size: u32) -> Option<TileId>;
    fn set_clip_rect(&mut self, rect: egui::Rect);

    fn position(&self, adjusted_pos: AdjustedPosition) -> Position {
        self.pixel_unproject(self.pixel_project(adjusted_pos.position) - adjusted_pos.offset)
    }
    //
    fn zero_offset(&self, adjusted_pos: AdjustedPosition) -> AdjustedPosition {
        AdjustedPosition {
            position: self.position(adjusted_pos),
            offset: Default::default(),
        }
    }
    // helpers
    fn to_screen_coords(&self, pos: Pixel) -> egui::Pos2;
    fn from_screen_coords(&self, screen_pos: egui::Pos2) -> Pixel;
    fn pixel_project(&self, pos: Position) -> Pixel;
    fn pixel_unproject(&self, pixel: Pixel) -> Position;
}

#[derive(Clone)]
pub(crate) struct LocalProjector {
    pub(crate) clip_rect: egui::Rect,
    pub(crate) memory: MapMemory,
    pub(crate) my_position: Position,
}

impl LocalProjector {
    fn units_per_point(zoom: f64) -> f64 {
        0.001 * 2_f64.powf(20. - zoom)
    }

    pub fn new(map_memory: &MapMemory, my_position: Position) -> Self {
        Self {
            clip_rect: egui::Rect::NOTHING,
            memory: map_memory.to_owned(),
            my_position,
        }
    }
}

impl ProjectorTrait for LocalProjector {
    fn scale_pixel_per_meter(&self, _position: Position) -> f32 {
        Self::units_per_point(self.memory.zoom()) as f32
    }

    fn project(&self, position: Position) -> egui::Pos2 {
        let zoom = self.memory.zoom();
        let units_per_point = Self::units_per_point(zoom);

        Pixel::new(
            position.x() / units_per_point,
            position.y() / units_per_point,
        )
    }

    fn unproject(&self, position: egui::Pos2) -> Position {
        // local pixel units
        let zoom = self.memory.zoom();
        let units_per_point = Self::units_per_point(zoom);

        Position::new(
            position.x() * units_per_point,
            position.y() * units_per_point,
        )
    }

    fn set_clip_rect(&mut self, rect: egui::Rect) {
        self.clip_rect = rect;
    }

    fn tile_id(&self, _pos: Position, _zoom: u8, _tile_size: u32) -> Option<TileId> {
        None
    }

    /// projects local coords into screen coords
    fn to_screen_coords(&self, pos: Pixel) -> egui::Pos2 {
        let map_center_projected_position =
            self.project(self.memory.center_mode.position(self.my_position, self));

        // From the two points above we can calculate the actual point on the screen.
        self.clip_rect.center() + (pos - map_center_projected_position).into()
    }

    /// projects local coords into flat mercator projection
    fn from_screen_coords(&self, pos: egui::Pos2) -> Pixel {
        let map_center_projected_position =
            self.project(self.memory.center_mode.position(self.my_position, self));

        map_center_projected_position + (pos - self.clip_rect.center()).into()
    }

    fn pixel_project(&self, pos: Position) -> Pixel {
        todo!()
    }

    fn pixel_unproject(&self, pixel: Pixel) -> Position {
        todo!()
    }
}

use std::f64::consts::PI;
// zoom level   tile coverage  number of tiles  tile size(*) in degrees
// 0            1 tile         1 tile           360° x 170.1022°
// 1            2 × 2 tiles    4 tiles          180° x 85.0511°
// 2            4 × 4 tiles    16 tiles         90° x [variable]
/// Zoom specifies how many pixels are in the whole map. For example, zoom 0 means that the whole
/// map is just one 256x256 tile, zoom 1 means that it is 2x2 tiles, and so on.
pub(crate) fn total_pixels(zoom: f64) -> f64 {
    2f64.powf(zoom) * (crate::TILE_SIZE as f64)
}

/// Project the lat/lon coordinates into a 2D x/y using the Web Mercator.
/// <https://en.wikipedia.org/wiki/Web_Mercator_projection>
/// <https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames>
/// <https://www.netzwolf.info/osm/tilebrowser.html?lat=51.157800&lon=6.865500&zoom=14>
#[derive(Clone)]
pub(crate) struct GlobalProjector {
    pub(crate) clip_rect: egui::Rect,
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

    pub fn new(map_memory: &MapMemory, my_position: Position) -> Self {
        Self {
            clip_rect: egui::Rect::NOTHING,
            memory: map_memory.to_owned(),
            my_position,
        }
    }
}

impl ProjectorTrait for GlobalProjector {
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
    fn project(&self, position: Position) -> egui::Pos2 {
        let zoom = self.memory.zoom();

        let total_pixels = total_pixels(zoom);

        // Turn that into a flat, mercator projection.
        let (x, y) = Self::mercator_normalized(position);

        Pixel::new(x * total_pixels, y * total_pixels)
    }

    /// unprojects flat mercator into lat lon
    fn unproject(&self, screen_pos: egui::Pos2) -> Position {
        let zoom = self.memory.zoom();
        let map_center = self.memory.center_mode.position(self.my_position, self);

        self.position(
            AdjustedPosition {
                position: map_center,
                offset: Default::default(),
            }
            .shift(-screen_pos.to_vec2()),
        )

        /*
        // for pixel
        let number_of_pixels: f64 = 2f64.powf(self.memory.zoom()) * (crate::TILE_SIZE as f64);

        let lon = screen_pos.x as f64;
        let lon = lon / number_of_pixels;
        let lon = (lon * 2. - 1.) * PI;
        let lon = lon.to_degrees();

        let lat = screen_pos.y as f64;
        let lat = lat / number_of_pixels;
        let lat = (-lat * 2. + 1.) * PI;
        let lat = lat.sinh().atan().to_degrees();

        Position::from_lon_lat(lon, lat)
        */
    }

    fn set_clip_rect(&mut self, rect: egui::Rect) {
        self.clip_rect = rect;
    }

    fn tile_id(&self, pos: Position, mut zoom: u8, source_tile_size: u32) -> Option<TileId> {
        let (x, y) = Self::mercator_normalized(pos);

        // Some sources provide larger tiles, effectively bundling e.g. 4 256px tiles in one
        // 512px one. Walkers uses 256px internally, so we need to adjust the zoom level.
        zoom -= (source_tile_size as f64 / TILE_SIZE as f64).log2() as u8;

        // Map that into a big bitmap made out of web tiles.
        let number_of_tiles = 2u32.pow(zoom as u32) as f64;
        let x = (x * number_of_tiles).floor() as u32;
        let y = (y * number_of_tiles).floor() as u32;

        Some(TileId { x, y, zoom })
    }

    /// projects flat mercator projection into screen coords
    fn to_screen_coords(&self, pos: Pixel) -> egui::Pos2 {
        let map_center_projected_position =
            self.project(self.memory.center_mode.position(self.my_position, self));

        // From the two points above we can calculate the actual point on the screen.
        self.clip_rect.center().to_vec2() + (pos - map_center_projected_position).into()
    }

    /// projects screen coords into flat mercator projection
    fn from_screen_coords(&self, pos: egui::Pos2) -> Pixel {
        let map_center_projected_position =
            self.project(self.memory.center_mode.position(self.my_position, self));

        map_center_projected_position + (pos - self.clip_rect.center().to_vec2()).into()
    }

    fn pixel_project(&self, pos: Position) -> Pixel {
        todo!()
    }

    fn pixel_unproject(&self, pixel: Pixel) -> Position {
        todo!()
    }
}
