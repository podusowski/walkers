use std::ops::{Add, Sub};

/// Position in some coordinates, either latitude and longitude or local projected coordinate system.
#[derive(Debug, Clone, Copy, Default)]
pub struct Position(geo_types::Point);

impl Position {
    /// Construct from latitude and longitude.
    pub fn from_lat_lon(lat: f64, lon: f64) -> Self {
        Self(geo_types::Point::new(lon, lat))
    }

    /// Construct from longitude and latitude. Note that it is common standard to write coordinates
    /// starting with the latitude instead (e.g. `51.104465719934176, 17.075169894118684` is
    /// the [Wrocław's zoo](https://zoo.wroclaw.pl/en/)).
    pub fn from_lon_lat(lon: f64, lat: f64) -> Self {
        Self(geo_types::Point::new(lon, lat))
    }

    pub fn new(x: f64, y: f64) -> Self {
        Self(geo_types::Point::new(x, y))
    }

    pub fn x(&self) -> f64 {
        self.0.x()
    }

    pub fn y(&self) -> f64 {
        self.0.y()
    }

    pub fn lat(&self) -> f64 {
        self.0.y()
    }

    pub fn lon(&self) -> f64 {
        self.0.x()
    }
}

impl From<geo_types::Point> for Position {
    fn from(value: geo_types::Point) -> Self {
        Self(value)
    }
}

impl From<Position> for geo_types::Point {
    fn from(value: Position) -> Self {
        value.0
    }
}

/// Location projected on the screen or an abstract bitmap.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Pixel(geo_types::Point);

impl Pixel {
    pub(crate) fn new(x: f64, y: f64) -> Pixel {
        Pixel(geo_types::Point::new(x, y))
    }

    pub(crate) fn x(&self) -> f64 {
        self.0.x()
    }

    pub(crate) fn y(&self) -> f64 {
        self.0.y()
    }
}

impl From<egui::Pos2> for Pixel {
    fn from(value: egui::Pos2) -> Self {
        Pixel::new(value.x as f64, value.y as f64)
    }
}

impl From<egui::Vec2> for Pixel {
    fn from(value: egui::Vec2) -> Self {
        Pixel::new(value.x as f64, value.y as f64)
    }
}

impl From<Pixel> for egui::Vec2 {
    fn from(val: Pixel) -> Self {
        egui::Vec2::new(val.x() as f32, val.y() as f32)
    }
}

impl Add<egui::Vec2> for Pixel {
    type Output = Self;

    fn add(self, rhs: egui::Vec2) -> Self::Output {
        Self::new(self.x() + rhs.x as f64, self.y() + rhs.y as f64)
    }
}

impl Sub<egui::Vec2> for Pixel {
    type Output = Self;

    fn sub(self, rhs: egui::Vec2) -> Self::Output {
        Self::new(self.x() - rhs.x as f64, self.y() - rhs.y as f64)
    }
}

impl Add for Pixel {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x() + rhs.x(), self.y() + rhs.y())
    }
}

impl Sub for Pixel {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x() - rhs.x(), self.y() - rhs.y())
    }
}

/// [`Position`] alone is not able to represent detached (e.g. after map gets dragged) position
/// due to insufficient accuracy.
#[derive(Debug, Clone)]
pub(crate) struct AdjustedPosition {
    /// Base geographical position.
    pub(crate) position: Position,

    /// Offset in pixels.
    pub(crate) offset: Pixel,
}

impl AdjustedPosition {
    pub(crate) fn new(position: Position, offset: Pixel) -> Self {
        Self { position, offset }
    }

    pub(crate) fn shift(self, shift: egui::Vec2) -> Self {
        Self {
            position: self.position,
            offset: self.offset + shift,
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
