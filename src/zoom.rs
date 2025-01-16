#[derive(thiserror::Error, Debug, PartialEq, Eq)]
#[error("invalid zoom level")]
pub struct InvalidZoom;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Zoom(f64);

impl TryFrom<f64> for Zoom {
    type Error = InvalidZoom;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        // The upper limit is artificial. Should it be removed altogether?
        if !(0. ..=26.).contains(&value) {
            Err(InvalidZoom)
        } else {
            Ok(Self(value))
        }
    }
}

// The reverse shouldn't be implemented, since we already have TryInto<f32>.
#[allow(clippy::from_over_into)]
impl Into<f64> for Zoom {
    fn into(self) -> f64 {
        self.0
    }
}

impl Default for Zoom {
    fn default() -> Self {
        Self(16.)
    }
}

impl Zoom {
    pub fn round(&self) -> u8 {
        self.0.round() as u8
    }

    pub fn zoom_in(&mut self) -> Result<(), InvalidZoom> {
        *self = Self::try_from(self.0 + 1.)?;
        Ok(())
    }

    pub fn zoom_out(&mut self) -> Result<(), InvalidZoom> {
        *self = Self::try_from(self.0 - 1.)?;
        Ok(())
    }

    /// Zoom using a relative value.
    pub fn zoom_by(&mut self, value: f64) {
        if let Ok(new_self) = Self::try_from(self.0 + value) {
            *self = new_self;
        }
    }
}
