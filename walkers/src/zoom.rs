#[derive(thiserror::Error, Debug, PartialEq, Eq)]
#[error("invalid zoom level")]
pub struct InvalidZoom;

#[derive(Debug, Clone)]
pub(crate) struct Zoom {
    value: f32,
    limits: Vec<u8>,
}

impl Default for Zoom {
    fn default() -> Self {
        Self {
            value: 16.,
            // Mapnik supports zooms up to 19.
            // https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames#Zoom_levels
            limits: Vec::from_iter(0..=19),
        }
    }
}

impl Zoom {
    pub fn limit_set(&mut self, limits: Vec<u8>) {
        self.limits = limits;
        self.zoom_set(self.value).unwrap_or_default();
    }

    pub fn round(&self) -> u8 {
        self.value.round() as u8
    }

    pub fn zoom_in(&mut self) -> Result<(), InvalidZoom> {
        self.zoom_set(self.value + 1.)?;
        Ok(())
    }

    pub fn zoom_out(&mut self) -> Result<(), InvalidZoom> {
        self.zoom_set(self.value - 1.)?;
        Ok(())
    }

    /// Zoom using a relative value.
    pub fn zoom_by(&mut self, value: f32) {
        self.zoom_set(self.value + value).unwrap_or_default();
    }

    /// Zoom using a absolute value.
    pub fn zoom_set(&mut self, value: f32) -> Result<Self, InvalidZoom> {
        if value < (*self.limits.first().unwrap() as f32) {
            self.value = *self.limits.first().unwrap() as f32;
            Err(InvalidZoom)
        } else if value > (*self.limits.last().unwrap() as f32) {
            self.value = *self.limits.last().unwrap() as f32;
            Err(InvalidZoom)
        } else if !self.limits.contains(&(value.round() as u8)) {
            let rounded_value = value.round() as u8;

            for idx in 1..self.limits.len() {
                if rounded_value > self.limits[idx - 1] && rounded_value < self.limits[idx] {
                    self.value = self.limits[idx - 1] as f32;
                    break;
                }
            }
            Ok(self.clone())
        } else {
            self.value = value;
            Ok(self.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constructing_zoom() {
        assert_eq!(16, Zoom::default().round());
        assert_eq!(19, Zoom::default().zoom_set(19.).unwrap().round());
        assert_eq!(InvalidZoom, Zoom::default().zoom_set(20.).unwrap_err());
    }

    #[test]
    fn test_zooming_in() {
        let mut zoom = Zoom::default().zoom_set(18.).unwrap();
        assert!(zoom.zoom_in().is_ok());
        assert_eq!(19, zoom.round());
        assert_eq!(Err(InvalidZoom), zoom.zoom_in());
    }

    #[test]
    fn test_zooming_out() {
        let mut zoom = Zoom::default().zoom_set(1.).unwrap();
        assert!(zoom.zoom_out().is_ok());
        assert_eq!(0, zoom.round());
        assert_eq!(Err(InvalidZoom), zoom.zoom_out());
    }

    #[test]
    fn test_set_limit() {
        let mut zoom = Zoom::default();
        zoom.zoom_by(16.0);
        zoom.limit_set(Vec::from_iter(0..=14));
        assert_eq!(zoom.round(), 14);
        zoom.limit_set(Vec::from_iter(16..=21));
        assert_eq!(zoom.round(), 16);
        zoom.zoom_by(4.4);
        zoom.limit_set(vec![17, 19, 23, 24]);
        assert_eq!(zoom.round(), 19);
    }
}
