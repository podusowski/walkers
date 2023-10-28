use crate::{Plugin, Position};
use egui::TextureId;
use egui::{pos2, Color32, ColorImage, Context, Rect, TextureHandle};
use std::cmp;

/// A image to be drawn on the map.
pub struct Image {
    /// Geographical position.
    pub position: Position,

    /// Texture id of image.
    pub texture: Texture,
}

/// [`Plugin`] which draws given list of images on the map.
pub struct Images {
    images: Vec<Image>,
}

#[derive(Clone)]
pub struct Texture {
    img: ColorImage,
    scaled_img: ColorImage,
    texture: TextureHandle,
    x_scale: f32,
    y_scale: f32,
    angle: f32,
}

impl Images {
    pub fn new(images: Vec<Image>) -> Self {
        Self { images }
    }
}

impl Plugin for Images {
    fn draw(&self, painter: egui::Painter, projector: &crate::Projector) {
        for image in &self.images {
            let screen_position = projector.project(image.position);
            let map_rect = painter.clip_rect();
            let texture = &image.texture;

            let [w, h] = texture.size();
            let w = w as f32;
            let h = h as f32;
            let mut rect = map_rect.translate(screen_position);

            rect.min.x -= w / 2.0;
            rect.min.y -= h / 2.0;

            rect.max.x = rect.min.x + w;
            rect.max.y = rect.min.y + h;

            let skip = (rect.max.x < map_rect.min.x)
                | (rect.max.y < map_rect.min.y)
                | (rect.min.x > map_rect.max.x)
                | (rect.min.y > map_rect.max.y);

            if skip {
                continue;
            }

            painter.image(
                texture.id(),
                rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }
}

trait ByPixel {
    fn get_pixel(&self, x: usize, y: usize) -> Option<Color32>;
    fn set_pixel(&mut self, x: usize, y: usize, color: Color32);
    fn index_to_xy(&self, i: usize) -> (usize, usize);
}

impl ByPixel for ColorImage {
    fn get_pixel(&self, x: usize, y: usize) -> Option<Color32> {
        let [w, h] = self.size;
        if x >= w || y >= h {
            return None;
        }
        Some(self.pixels[x + w * y])
    }

    fn set_pixel(&mut self, x: usize, y: usize, color: Color32) {
        let [w, h] = self.size;
        if x >= w || y >= h {
            return;
        }
        self.pixels[x + w * y] = color;
    }

    fn index_to_xy(&self, i: usize) -> (usize, usize) {
        let w = self.size[0];
        let y = i / w;
        let x = i % w;
        (x, y)
    }
}

impl Texture {
    /// Construct new texture
    /// ⚠️ Make sure to only call this ONCE for each image, i.e. NOT in your main GUI code.
    /// The call is NOT immediate safe.
    pub fn new(ctx: Context, uri: &str, img: ColorImage) -> Self {
        let texture = ctx.load_texture(uri, img.clone(), Default::default());

        Self {
            img: img.clone(),
            scaled_img: img,
            texture,
            x_scale: 1.0,
            y_scale: 1.0,
            angle: 0.0,
        }
    }

    /// Same as [egui::TextureHandle::id]
    /// (https://docs.rs/egui/latest/egui/struct.TextureHandle.html#method.id)
    #[inline(always)]
    pub fn id(&self) -> TextureId {
        self.texture.id()
    }

    /// Same as [egui::TextureHandle::size] (https://docs.rs/egui/latest/egui/struct.TextureHandle.html#method.size)
    #[inline(always)]
    pub fn size(&self) -> [usize; 2] {
        self.texture.size()
    }

    /// Scale texture.
    pub fn scale(&mut self, x_val: f32, y_val: f32) {
        if x_val == self.x_scale && y_val == self.y_scale {
            return;
        }
        self.x_scale = x_val;
        self.y_scale = y_val;
        self.do_scale();
    }

    /// Rotate texture.
    pub fn rotate(&mut self, degree: f32) {
        if degree == self.angle {
            return;
        }
        self.angle = degree;
        self.do_rotate();
    }

    // This code based on transform::rotate from [raster] (https://crates.io/crates/raster) crate
    fn do_rotate(&mut self) {
        let degree = self.angle;
        let src = &self.scaled_img;
        let [w1, h1] = src.size;
        let w1 = w1 as i32;
        let h1 = h1 as i32;
        // Using screen coords system, top left is always at (0,0)
        let mut min_x = 0;
        let mut max_x = 0;
        let mut min_y = 0;
        let mut max_y = 0;

        let top_right_1 = (w1, 0);
        let top_right_2 = _rotate(top_right_1, degree);
        min_x = cmp::min(min_x, top_right_2.0);
        max_x = cmp::max(max_x, top_right_2.0);
        min_y = cmp::min(min_y, top_right_2.1);
        max_y = cmp::max(max_y, top_right_2.1);

        let bottom_right_1 = (w1, h1);
        let bottom_right_2 = _rotate(bottom_right_1, degree);
        min_x = cmp::min(min_x, bottom_right_2.0);
        max_x = cmp::max(max_x, bottom_right_2.0);
        min_y = cmp::min(min_y, bottom_right_2.1);
        max_y = cmp::max(max_y, bottom_right_2.1);

        let bottom_left_1 = (0, h1);
        let bottom_left_2 = _rotate(bottom_left_1, degree);
        min_x = cmp::min(min_x, bottom_left_2.0);
        max_x = cmp::max(max_x, bottom_left_2.0);
        min_y = cmp::min(min_y, bottom_left_2.1);
        max_y = cmp::max(max_y, bottom_left_2.1);

        let w2 = ((min_x as f32).abs() + (max_x as f32).abs()) as i32 + 1;
        let h2 = ((min_y as f32).abs() + (max_y as f32).abs()) as i32 + 1;

        let mut dest = ColorImage::new([w2 as usize, h2 as usize], Color32::TRANSPARENT);

        for (dest_y, y) in (0..).zip(min_y..max_y + 1) {
            for (dest_x, x) in (0..).zip(min_x..max_x + 1) {
                let point: (i32, i32) = _rotate((x, y), -degree);

                if point.0 >= 0 && point.0 < w1 && point.1 >= 0 && point.1 < h1 {
                    if let Some(pixel) = src.get_pixel(point.0 as usize, point.1 as usize) {
                        dest.set_pixel(dest_x, dest_y, pixel);
                    }
                }
            }
        }

        self.texture.set(dest, egui::TextureOptions::LINEAR);
    }

    fn do_scale(&mut self) {
        let x_ratio = self.x_scale;
        let y_ratio = self.y_scale;

        let [w, h] = self.img.size;
        let w = (w as f32 * x_ratio) as usize;
        let h = (h as f32 * y_ratio) as usize;

        self.scaled_img = ColorImage::new([w, h], Color32::TRANSPARENT);
        for y in 0..h {
            for x in 0..w {
                let px = (x as f32 / x_ratio).floor() as usize;
                let py = (y as f32 / y_ratio).floor() as usize;
                if let Some(pixel) = self.img.get_pixel(px, py) {
                    self.scaled_img.set_pixel(x, y, pixel);
                }
            }
        }

        self.texture
            .set(self.scaled_img.clone(), egui::TextureOptions::LINEAR);
    }
}

fn _rotate(p: (i32, i32), deg: f32) -> (i32, i32) {
    let radians: f32 = deg.to_radians();
    let px: f32 = p.0 as f32;
    let py: f32 = p.1 as f32;
    let cos = radians.cos();
    let sin = radians.sin();
    let x = ((px * cos) - (py * sin)).round();
    let y = ((px * sin) + (py * cos)).round();
    (x as i32, y as i32)
}
