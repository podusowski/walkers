use emath::Rect;
#[cfg(feature = "vector")]
use emath::TSTransform;
#[cfg(feature = "vector")]
use std::sync::Arc;

use crate::ZOrder;
use crate::tiles::RasterImage;
#[cfg(feature = "vector")]
use crate::{Drawable, Symbol, Text};

/// Everything the map's layers draw in a frame, with labels on top. It does not know how to
/// paint itself, that is up to a backend, such as egui's.
pub struct Canvas {
    /// In the order they are drawn.
    pub(crate) items: Vec<Item>,

    #[cfg(feature = "vector")]
    pub(crate) symbols: Vec<(ZOrder, Symbol)>,

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
#[derive(Clone, Debug)]
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

    /// The area they cover, in their own coordinates.
    pub(crate) fn bounds(&self) -> Rect {
        self.drawables
            .iter()
            .fold(Rect::NOTHING, |bounds, drawable| {
                bounds.union(drawable.bounds())
            })
    }
}

impl Canvas {
    pub(crate) fn new() -> Self {
        Self {
            items: Vec::new(),
            #[cfg(feature = "vector")]
            symbols: Vec::new(),
            z_order: ZOrder::default(),
            transparency: 1.0,
        }
    }

    /// Seed for the texts of the layer being drawn.
    pub fn z_order(&self) -> ZOrder {
        self.z_order
    }

    /// `symbol` is in screen coordinates.
    #[cfg(feature = "vector")]
    pub fn add_symbol(&mut self, z_order: ZOrder, symbol: Symbol) {
        let transparency = self.transparency;
        let symbol = Symbol {
            text: symbol.text.map(|text| Text {
                text_color: text.text_color.gamma_multiply(transparency),
                halo_color: text.halo_color.gamma_multiply(transparency),
                ..text
            }),
            icon: symbol.icon.map(|icon| {
                let transparency = icon.transparency * transparency;
                icon.with_transparency(transparency)
            }),
            ..symbol
        };
        self.symbols.push((z_order, symbol));
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

    #[test]
    fn transparency_of_the_layer_is_applied_to_the_symbols_own() {
        let mut canvas = Canvas {
            transparency: 0.5,
            ..Canvas::new()
        };
        let symbol = Symbol::new(emath::pos2(0., 0.))
            .with_text(Text::new("Śnieżka".into(), 12., ecolor::Color32::WHITE, 0.))
            .with_icon(Drawables::new(Vec::new()).with_transparency(0.5));

        canvas.add_symbol(ZOrder::default(), symbol);

        match canvas.symbols.as_slice() {
            [(_, symbol)] => {
                assert_eq!(
                    symbol.text.as_ref().map(|text| text.text_color),
                    Some(ecolor::Color32::WHITE.gamma_multiply(0.5))
                );
                assert_eq!(
                    symbol.icon.as_ref().map(|icon| icon.transparency),
                    Some(0.25)
                );
            }
            _ => panic!("expected a single symbol"),
        }
    }
}
