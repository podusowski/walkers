use std::ops::Deref;

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
#[error("invalid zoom level")]
pub struct InvalidZoom;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Zoom(u8);

impl TryFrom<u8> for Zoom {
    type Error = InvalidZoom;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        // Mapnik supports up to 19.
        // https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames#Zoom_levels
        if value > 19 {
            Err(InvalidZoom)
        } else {
            Ok(Self(value))
        }
    }
}

impl Default for Zoom {
    fn default() -> Self {
        Self(16)
    }
}

impl Deref for Zoom {
    type Target = u8;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Zoom {
    pub fn zoom_in(&mut self) -> Result<(), InvalidZoom> {
        *self = Self::try_from(self.0 + 1)?;
        Ok(())
    }

    pub fn zoom_out(&mut self) -> Result<(), InvalidZoom> {
        *self = Self::try_from(self.0.checked_sub(1).ok_or(InvalidZoom)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constructing_zoom() {
        assert_eq!(16, *Zoom::default());
        assert_eq!(19, *Zoom::try_from(19).unwrap());
        assert_eq!(Err(InvalidZoom), Zoom::try_from(20));
    }

    #[test]
    fn test_zooming_in() {
        let mut zoom = Zoom::try_from(18).unwrap();
        assert!(zoom.zoom_in().is_ok());
        assert_eq!(19, *zoom);
        assert_eq!(Err(InvalidZoom), zoom.zoom_in());
    }

    #[test]
    fn test_zooming_out() {
        let mut zoom = Zoom::try_from(1).unwrap();
        assert!(zoom.zoom_out().is_ok());
        assert_eq!(0, *zoom);
        assert_eq!(Err(InvalidZoom), zoom.zoom_out());
    }
}
