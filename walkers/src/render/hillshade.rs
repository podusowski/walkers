//! Shading elevation into relief, the way MapLibre's `standard` hillshade method does, only once
//! per tile rather than on the GPU.
//!
//! <https://github.com/maplibre/maplibre-gl-js/blob/v4.7.1/src/shaders/hillshade.fragment.glsl>

use std::collections::HashMap;
use std::f32::consts::PI;

use ecolor::Color32;
use image::RgbaImage;

use crate::{
    DemEncoding, TileId,
    expression::Context,
    style::{Color, Float, Paint},
};

const EARTH_CIRCUMFERENCE: f32 = 40_075_016.686;

/// Shade the elevation `image` with each of `paints`, the later ones on top. Pixels come out
/// premultiplied, row by row.
pub(crate) fn render(
    image: &RgbaImage,
    encoding: DemEncoding,
    paints: &[&Paint],
    tile_id: TileId,
) -> Vec<Color32> {
    let (width, height) = (image.width() as usize, image.height() as usize);
    let elevations: Vec<f32> = image
        .pixels()
        .map(|pixel| elevation(encoding, pixel.0))
        .collect();
    let slopes = slopes(&elevations, width, height, tile_id.zoom);
    let mut shaded = vec![[0.0; 4]; width * height];

    for paint in paints {
        let light = Light::new(paint, tile_id.zoom);

        for (row, (slopes, shaded)) in slopes
            .chunks(width)
            .zip(shaded.chunks_mut(width))
            .enumerate()
        {
            // Mercator stretches the ground away from the equator, which flattens the slopes.
            let stretch = latitude(tile_id, (row as f32 + 0.5) / height as f32)
                .to_radians()
                .cos();

            for (slope, shaded) in slopes.iter().zip(shaded) {
                *shaded = over(light.shade(*slope, stretch), *shaded);
            }
        }
    }

    shaded
        .into_iter()
        .map(|[r, g, b, a]| {
            let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
            Color32::from_rgba_premultiplied(channel(r), channel(g), channel(b), channel(a))
        })
        .collect()
}

/// In meters.
fn elevation(encoding: DemEncoding, [r, g, b, _]: [u8; 4]) -> f32 {
    let (r, g, b) = (r as f32, g as f32, b as f32);
    match encoding {
        DemEncoding::Mapbox => (r * 65536.0 + g * 256.0 + b) * 0.1 - 10000.0,
        DemEncoding::Terrarium => r * 256.0 + g + b / 256.0 - 32768.0,
    }
}

/// How steeply the ground rises east and south of each pixel. Pixels at the edges of the tile
/// borrow their missing neighbors from themselves, as the tiles around are not at hand.
fn slopes(elevations: &[f32], width: usize, height: usize, zoom: u8) -> Vec<[f32; 2]> {
    let at = |x: usize, y: usize, dx: isize, dy: isize| {
        let x = x.saturating_add_signed(dx).min(width - 1);
        let y = y.saturating_add_signed(dy).min(height - 1);
        // MapLibre quarters the elevation, and its look is tuned for that.
        elevations[y * width + x] / 4.0
    };

    // Relief is barely noticeable when zoomed out, so MapLibre exaggerates it there.
    let zoom = zoom as f32;
    let factor = if zoom < 2.0 {
        0.4
    } else if zoom < 4.5 {
        0.35
    } else {
        0.3
    };
    let exaggeration = if zoom < 15.0 {
        (zoom - 15.0) * factor
    } else {
        0.0
    };

    // Eight pixels, as that is what the weights of the Sobel operator add up to.
    let pixel_size = EARTH_CIRCUMFERENCE / (width as f32 * 2f32.powf(zoom));
    let divisor = 8.0 * pixel_size * 2f32.powf(exaggeration);

    (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .map(|(x, y)| {
            let e = |dx, dy| at(x, y, dx, dy);
            let east =
                (e(1, -1) + 2.0 * e(1, 0) + e(1, 1)) - (e(-1, -1) + 2.0 * e(-1, 0) + e(-1, 1));
            let south =
                (e(-1, 1) + 2.0 * e(0, 1) + e(1, 1)) - (e(-1, -1) + 2.0 * e(0, -1) + e(1, -1));

            // MapLibre keeps them in a texture, which clamps them.
            [
                (east / divisor).clamp(-1.0, 1.0),
                (south / divisor).clamp(-1.0, 1.0),
            ]
        })
        .collect()
}

/// Of the point `fraction` of the way down the tile.
fn latitude(tile_id: TileId, fraction: f32) -> f32 {
    let y = (tile_id.y as f32 + fraction) / 2f32.powi(tile_id.zoom as i32);
    (PI * (1.0 - 2.0 * y)).sinh().atan().to_degrees()
}

struct Light {
    /// In radians, where the light comes from, clockwise from the north.
    direction: f32,
    exaggeration: f32,
    shadow: [f32; 4],
    highlight: [f32; 4],
    accent: [f32; 4],
}

impl Light {
    fn new(paint: &Paint, zoom: u8) -> Self {
        let context = Context::new("None".to_string(), HashMap::new(), zoom);
        let float = |value: &Option<Float>, default: f32| {
            value
                .as_ref()
                .map_or(default, |value| value.evaluate(&context))
        };
        let color = |value: &Option<Color>, default: Color32| {
            let color = value
                .as_ref()
                .map_or(default, |value| value.evaluate(&context));
            color.to_array().map(|channel| channel as f32 / 255.0)
        };

        Self {
            direction: float(&paint.hillshade_illumination_direction, 335.0).to_radians(),
            exaggeration: float(&paint.hillshade_exaggeration, 0.5),
            shadow: color(&paint.hillshade_shadow_color, Color32::BLACK),
            highlight: color(&paint.hillshade_highlight_color, Color32::WHITE),
            accent: color(&paint.hillshade_accent_color, Color32::BLACK),
        }
    }

    /// Premultiplied color of the ground rising by `east` and `south`, `stretch` being how much
    /// Mercator flattened it.
    fn shade(&self, [east, south]: [f32; 2], stretch: f32) -> [f32; 4] {
        // 1.25 is MapLibre's, just as arbitrary as it looks.
        let slope = (1.25 * east.hypot(south) / stretch).atan();
        let aspect = if east != 0.0 {
            south.atan2(-east)
        } else {
            PI / 2.0 * south.signum()
        };

        // Exaggeration bends the slope, so that the steep ones stand out more, or less.
        let base = 1.875 - self.exaggeration * 1.75;
        let max = PI / 2.0;
        let slope = if (base - 1.0).abs() > f32::EPSILON {
            (base.powf(slope) - 1.0) / (base.powf(max) - 1.0) * max
        } else {
            slope
        };

        // Below 0.5, exaggeration fades the shading out.
        let strength = (self.exaggeration * 2.0).clamp(0.0, 1.0);

        let accent = scale(self.accent, (1.0 - slope.cos()) * strength);
        let facing = (((aspect + self.direction + PI) / PI + 0.5).rem_euclid(2.0) - 1.0).abs();
        let shade = scale(
            lerp(self.shadow, self.highlight, facing),
            slope.sin() * strength,
        );

        over(shade, accent)
    }
}

fn scale(color: [f32; 4], factor: f32) -> [f32; 4] {
    color.map(|channel| channel * factor)
}

fn lerp(from: [f32; 4], to: [f32; 4], t: f32) -> [f32; 4] {
    std::array::from_fn(|i| from[i] + (to[i] - from[i]) * t)
}

/// Premultiplied `top` drawn over `bottom`.
fn over(top: [f32; 4], bottom: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|i| top[i] + bottom[i] * (1.0 - top[3]))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    pub(crate) fn terrarium(meters: f32) -> image::Rgba<u8> {
        let value = meters + 32768.0;
        let r = (value / 256.0).floor();
        let g = (value - r * 256.0).floor();
        let b = ((value - r * 256.0 - g) * 256.0).floor();
        image::Rgba([r as u8, g as u8, b as u8, 255])
    }

    const TILE: TileId = TileId {
        x: 2240,
        y: 1393,
        zoom: 12,
    };

    /// A tile whose ground rises by `east` and `south` meters with every pixel that way.
    pub(crate) fn ramp(east: f32, south: f32) -> RgbaImage {
        RgbaImage::from_fn(16, 16, |x, y| {
            terrarium(1000.0 + x as f32 * east + y as f32 * south)
        })
    }

    fn shade(image: &RgbaImage, paint: &Paint) -> Vec<Color32> {
        render(image, DemEncoding::Terrarium, &[paint], TILE)
    }

    fn middle(pixels: &[Color32]) -> Color32 {
        pixels[8 * 16 + 8]
    }

    /// Brightness of shading which is black and white, given how opaque white is.
    fn brightness(color: Color32) -> f32 {
        color.r() as f32 / color.a().max(1) as f32
    }

    #[test]
    fn elevation_is_unpacked_from_either_encoding() {
        assert_eq!(elevation(DemEncoding::Terrarium, [128, 0, 0, 255]), 0.0);
        assert_eq!(
            elevation(DemEncoding::Terrarium, [131, 232, 128, 255]),
            1000.5
        );
        assert_eq!(elevation(DemEncoding::Mapbox, [1, 134, 160, 255]), 0.0);
        assert_eq!(elevation(DemEncoding::Mapbox, [1, 173, 176, 255]), 1000.0);
    }

    #[test]
    fn flat_ground_is_not_shaded() {
        let shaded = shade(&ramp(0.0, 0.0), &Paint::default());
        assert!(shaded.iter().all(|pixel| *pixel == Color32::TRANSPARENT));
    }

    /// The light comes from the north-northwest by default.
    #[test]
    fn slopes_facing_the_light_are_brighter_than_those_facing_away() {
        let paint = Paint::default();

        let facing_north = middle(&shade(&ramp(0.0, 20.0), &paint));
        let facing_south = middle(&shade(&ramp(0.0, -20.0), &paint));
        let facing_west = middle(&shade(&ramp(20.0, 0.0), &paint));
        let facing_east = middle(&shade(&ramp(-20.0, 0.0), &paint));

        assert!(brightness(facing_north) > brightness(facing_south));
        assert!(brightness(facing_west) > brightness(facing_east));
    }

    #[test]
    fn steeper_ground_is_shaded_more() {
        let paint = Paint::default();

        let gentle = middle(&shade(&ramp(0.0, -5.0), &paint));
        let steep = middle(&shade(&ramp(0.0, -50.0), &paint));

        assert!(steep.a() > gentle.a(), "{steep:?} vs {gentle:?}");
    }

    #[test]
    fn colors_come_from_the_paint() {
        let paint = Paint {
            hillshade_shadow_color: Some(Color(json!("#ff0000"))),
            hillshade_highlight_color: Some(Color(json!("#ff0000"))),
            hillshade_accent_color: Some(Color(json!("#ff0000"))),
            ..Default::default()
        };

        let shaded = middle(&shade(&ramp(0.0, -20.0), &paint));

        assert!(shaded.a() > 0);
        assert_eq!(shaded.r(), shaded.a());
        assert_eq!((shaded.g(), shaded.b()), (0, 0));
    }

    #[test]
    fn no_exaggeration_means_no_shading() {
        let paint = Paint {
            hillshade_exaggeration: Some(Float(json!(0.0))),
            ..Default::default()
        };

        let shaded = shade(&ramp(0.0, -20.0), &paint);
        assert!(shaded.iter().all(|pixel| *pixel == Color32::TRANSPARENT));
    }

    /// Edges have no neighbors in the tile, which must not take it down.
    #[test]
    fn a_single_pixel_tile_is_shaded() {
        let image = RgbaImage::from_pixel(1, 1, terrarium(1000.0));
        assert_eq!(shade(&image, &Paint::default()).len(), 1);
    }

    #[test]
    fn latitude_spans_the_tile() {
        let top = TileId {
            x: 0,
            y: 0,
            zoom: 0,
        };
        assert!((latitude(top, 0.0) - 85.0511).abs() < 0.001);
        assert!(latitude(top, 0.5).abs() < 0.001);
        assert!((latitude(top, 1.0) + 85.0511).abs() < 0.001);
    }
}
