use emath::Rect;
#[cfg(feature = "vector")]
use emath::TSTransform;
#[cfg(feature = "vector")]
use std::sync::Arc;

use crate::ZOrder;
use crate::tiles::RasterImage;
#[cfg(feature = "vector")]
use crate::{Drawable, Text};

/// Everything the map's layers draw in a frame, with labels on top. It does not know how to
/// paint itself, that is up to a backend, such as egui's.
pub struct Canvas {
    /// In the order they are drawn.
    pub(crate) items: Vec<Item>,

    #[cfg(feature = "vector")]
    pub(crate) texts: Vec<(ZOrder, Text)>,

    /// Of the layer being drawn.
    pub(crate) z_order: ZOrder,

    /// Of the layer being drawn, applied to everything it adds.
    pub(crate) transparency: f32,
}

/// Something put on the [`Canvas`], with the transparency of the layer it came from already
/// applied.
pub(crate) enum Item {
    #[cfg(feature = "vector")]
    Drawables(Drawables),
    Raster(RasterPiece),
}

/// Part of a raster image, stretched over a rectangle on the screen.
pub(crate) struct RasterPiece {
    pub image: RasterImage,
    pub rect: Rect,

    /// Which part of the image goes onto `rect`.
    pub uv: Rect,

    pub transparency: f32,
}

/// Drawables to be put on the [`Canvas`], and how. Cheap to clone, and drawing the same ones
/// again does not upload them to the GPU again.
#[cfg(feature = "vector")]
#[derive(Clone)]
pub struct Drawables {
    /// Behind an `Arc`, because that is what the renderer tells uploaded drawables by.
    pub(crate) drawables: Arc<Vec<Drawable>>,
    pub(crate) transform: TSTransform,
    pub(crate) clip_rect: Rect,
    pub(crate) transparency: f32,
}

#[cfg(feature = "vector")]
impl Drawables {
    /// Drawn as they are, in screen coordinates.
    pub fn new(drawables: Vec<Drawable>) -> Self {
        Self::merged(Arc::new(crate::render::merge_runs(drawables)))
    }

    /// For drawables which already went through [`crate::render::merge_runs`].
    pub(crate) fn merged(drawables: Arc<Vec<Drawable>>) -> Self {
        Self {
            drawables,
            transform: TSTransform::IDENTITY,
            // Clipped to the map anyway.
            clip_rect: Rect::EVERYTHING,
            transparency: 1.0,
        }
    }

    /// From the drawables' own coordinates onto the screen.
    pub fn with_transform(mut self, transform: TSTransform) -> Self {
        self.transform = transform;
        self
    }

    pub fn with_clip_rect(mut self, clip_rect: Rect) -> Self {
        self.clip_rect = clip_rect;
        self
    }

    pub fn with_transparency(mut self, transparency: f32) -> Self {
        self.transparency = transparency;
        self
    }
}

impl Canvas {
    pub(crate) fn new() -> Self {
        Self {
            items: Vec::new(),
            #[cfg(feature = "vector")]
            texts: Vec::new(),
            z_order: ZOrder::default(),
            transparency: 1.0,
        }
    }

    /// Seed for the texts of the layer being drawn.
    pub fn z_order(&self) -> ZOrder {
        self.z_order
    }

    /// `text` is in screen coordinates.
    #[cfg(feature = "vector")]
    pub fn add_text(&mut self, z_order: ZOrder, text: Text) {
        let text = Text {
            text_color: text.text_color.gamma_multiply(self.transparency),
            halo_color: text.halo_color.gamma_multiply(self.transparency),
            ..text
        };
        self.texts.push((z_order, text));
    }

    #[cfg(feature = "vector")]
    pub fn add_drawables(&mut self, drawables: Drawables) {
        self.items.push(Item::Drawables(Drawables {
            transparency: drawables.transparency * self.transparency,
            ..drawables
        }));
    }

    pub(crate) fn add_raster(&mut self, image: RasterImage, rect: Rect, uv: Rect) {
        self.items.push(Item::Raster(RasterPiece {
            image,
            rect,
            uv,
            transparency: self.transparency,
        }));
    }
}

#[cfg(all(test, feature = "vector"))]
mod tests {
    use super::*;

    #[test]
    fn transparency_of_the_layer_is_applied_to_the_drawables_own() {
        let mut canvas = Canvas {
            transparency: 0.5,
            ..Canvas::new()
        };

        canvas.add_drawables(Drawables::new(Vec::new()).with_transparency(0.5));

        match canvas.items.as_slice() {
            [Item::Drawables(drawables)] => assert_eq!(drawables.transparency, 0.25),
            _ => panic!("expected a single run of drawables"),
        }
    }
}
