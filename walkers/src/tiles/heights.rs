//! Elevation of a tile, in meters.

use image::RgbaImage;

use crate::DemEncoding;

/// Elevation of each pixel of a tile, in meters.
pub(crate) struct Heights {
    width: usize,
    height: usize,

    /// Row by row.
    values: Vec<f32>,
}

impl Heights {
    /// Of an encoded image, such as a PNG or a WebP, whose pixels are packed as `encoding` says.
    pub(crate) fn decode(data: &[u8], encoding: DemEncoding) -> Result<Self, image::ImageError> {
        Ok(Self::from_image(
            &image::load_from_memory(data)?.to_rgba8(),
            encoding,
        ))
    }

    pub(crate) fn from_image(image: &RgbaImage, encoding: DemEncoding) -> Self {
        Self {
            width: image.width() as usize,
            height: image.height() as usize,
            values: image
                .pixels()
                .map(|pixel| encoding.decode(pixel.0))
                .collect(),
        }
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }

    pub(crate) fn height(&self) -> usize {
        self.height
    }

    /// Row by row.
    pub(crate) fn values(&self) -> &[f32] {
        &self.values
    }
}

/// Elevation at the centers of the pixels, and at the edges of the tile, where it goes on as it
/// does next to them. In between, it changes linearly.
pub(crate) struct Grid {
    /// Row by row.
    values: Vec<f32>,

    /// Where the columns and rows are, in pixels from the top left corner of the tile.
    xs: Vec<f32>,
    ys: Vec<f32>,
}

impl Grid {
    /// None for a tile with no pixels.
    pub(crate) fn new(heights: &Heights) -> Option<Self> {
        if heights.width == 0 || heights.height == 0 {
            return None;
        }

        let rows: Vec<Vec<f32>> = heights.values.chunks(heights.width).map(extended).collect();

        let edge_row = |row: usize, next: usize| -> Vec<f32> {
            rows[row]
                .iter()
                .zip(&rows[next])
                .map(|(edge, next)| past(*edge, *next))
                .collect()
        };
        let last = rows.len() - 1;
        let top = edge_row(0, 1.min(last));
        let bottom = edge_row(last, last.saturating_sub(1));

        Some(Self {
            values: [top]
                .into_iter()
                .chain(rows)
                .chain([bottom])
                .flatten()
                .collect(),
            xs: positions(heights.width),
            ys: positions(heights.height),
        })
    }

    pub(crate) fn xs(&self) -> &[f32] {
        &self.xs
    }

    pub(crate) fn ys(&self) -> &[f32] {
        &self.ys
    }

    /// At the column `x` and the row `y`.
    pub(crate) fn at(&self, x: usize, y: usize) -> f32 {
        self.values[y * self.xs.len() + x]
    }

    /// At `x` and `y` pixels from the top left corner of the tile, which are clamped to it.
    pub(crate) fn sample(&self, x: f32, y: f32) -> f32 {
        let (column, across) = cell(&self.xs, x);
        let (row, down) = cell(&self.ys, y);

        let top = lerp(self.at(column, row), self.at(column + 1, row), across);
        let bottom = lerp(
            self.at(column, row + 1),
            self.at(column + 1, row + 1),
            across,
        );
        lerp(top, bottom, down)
    }
}

/// `values` with one more at each end, at the edge of the tile, half a pixel past them.
fn extended(values: &[f32]) -> Vec<f32> {
    let last = values.len() - 1;
    let first = past(values[0], values[1.min(last)]);
    let after = past(values[last], values[last.saturating_sub(1)]);
    [first]
        .into_iter()
        .chain(values.iter().copied())
        .chain([after])
        .collect()
}

/// Half a pixel past `edge`, going on from `next` to it.
fn past(edge: f32, next: f32) -> f32 {
    edge + (edge - next) / 2.0
}

/// Of the edges and of the centers of `pixels` pixels.
fn positions(pixels: usize) -> Vec<f32> {
    [0.0]
        .into_iter()
        .chain((0..pixels).map(|pixel| pixel as f32 + 0.5))
        .chain([pixels as f32])
        .collect()
}

/// The one between `positions` which `at` is in, and how far into it, from 0 to 1.
fn cell(positions: &[f32], at: f32) -> (usize, f32) {
    let last = positions.len() - 2;
    let index = positions[1..=last].partition_point(|position| *position <= at);
    let (from, to) = (positions[index], positions[index + 1]);
    (index, ((at - from) / (to - from)).clamp(0.0, 1.0))
}

fn lerp(from: f32, to: f32, fraction: f32) -> f32 {
    from + (to - from) * fraction
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn terrarium(meters: f32) -> image::Rgba<u8> {
        let value = meters + 32768.0;
        let r = (value / 256.0).floor();
        let g = (value - r * 256.0).floor();
        let b = ((value - r * 256.0 - g) * 256.0).floor();
        image::Rgba([r as u8, g as u8, b as u8, 255])
    }

    /// 1000 meters at the top left pixel, rising by 10 with every pixel east, and by 100 with
    /// every pixel south.
    pub(crate) fn tilted() -> RgbaImage {
        RgbaImage::from_fn(16, 16, |x, y| {
            terrarium(1000.0 + 10.0 * x as f32 + 100.0 * y as f32)
        })
    }

    fn grid() -> Grid {
        Grid::new(&Heights::from_image(&tilted(), DemEncoding::Terrarium)).unwrap()
    }

    #[test]
    fn pixels_are_read_at_their_centers() {
        assert_eq!(grid().sample(0.5, 0.5), 1000.0);
        assert_eq!(grid().sample(3.5, 2.5), 1230.0);
    }

    #[test]
    fn ground_between_pixels_is_interpolated() {
        assert_eq!(grid().sample(1.0, 0.5), 1005.0);
        assert_eq!(grid().sample(1.0, 1.0), 1055.0);
        assert_eq!(grid().sample(1.25, 0.75), 1032.5);
    }

    /// Half a pixel past the centers of those along them.
    #[test]
    fn ground_at_the_edges_goes_on_as_it_does_next_to_them() {
        assert_eq!(grid().sample(0.0, 0.5), 995.0);
        assert_eq!(grid().sample(16.0, 0.0), 1000.0 + 155.0 - 50.0);
    }

    #[test]
    fn past_the_tile_its_edges_are_read() {
        assert_eq!(grid().sample(-3.0, 0.5), 995.0);
        assert_eq!(grid().sample(100.0, 0.0), grid().sample(16.0, 0.0));
    }

    #[test]
    fn a_single_pixel_is_flat() {
        let image = RgbaImage::from_pixel(1, 1, terrarium(1000.0));
        let grid = Grid::new(&Heights::from_image(&image, DemEncoding::Terrarium)).unwrap();
        assert_eq!(grid.sample(0.0, 0.0), 1000.0);
        assert_eq!(grid.sample(0.7, 0.2), 1000.0);
    }

    #[test]
    fn a_tile_with_no_pixels_has_no_grid() {
        let heights = Heights::from_image(&RgbaImage::new(0, 0), DemEncoding::Terrarium);
        assert!(Grid::new(&heights).is_none());
    }
}
