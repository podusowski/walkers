use crate::{Plugin, Position};
use egui::epaint::emath::Rot2;
use egui::{pos2, Color32, ColorImage, Context, Rect, TextureHandle, TextureId};

/// An image to be drawn on the map.
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
    texture: TextureHandle,
    x_scale: f32,
    y_scale: f32,
    angle: Rot2,
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
            let w = w as f32 * texture.x_scale;
            let h = h as f32 * texture.y_scale;
            let mut rect = map_rect.translate(screen_position);

            rect.min.x -= w / 2.0;
            rect.min.y -= h / 2.0;

            rect.max.x = rect.min.x + w;
            rect.max.y = rect.min.y + h;

            if map_rect.intersects(rect) {
                let mut mesh = egui::Mesh::with_texture(texture.id());
                let angle = texture.angle;

                mesh.add_rect_with_uv(
                    rect,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );

                let origin = egui::Vec2::splat(0.5);
                mesh.rotate(angle, rect.min + origin * rect.size());
                painter.add(mesh);
            }
        }
    }
}

impl Texture {
    /// Construct new texture
    /// ⚠️ Make sure to only call this ONCE for each image, i.e. NOT in your main GUI code.
    /// The call is NOT immediate safe.
    pub fn new(ctx: Context, uri: &str, img: ColorImage) -> Self {
        let texture = ctx.load_texture(uri, img.clone(), Default::default());

        Self {
            texture,
            x_scale: 1.0,
            y_scale: 1.0,
            angle: Rot2::from_angle(0.0),
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
    #[inline(always)]
    pub fn scale(&mut self, x_val: f32, y_val: f32) {
        self.x_scale = x_val;
        self.y_scale = y_val;
    }

    /// Rotate texture.
    /// Angle is clockwise in radians. A 𝞃/4 = 90° rotation means rotating the X axis to the Y axis.
    #[inline(always)]
    pub fn angle(&mut self, angle: f32) {
        self.angle = Rot2::from_angle(angle);
    }
}
