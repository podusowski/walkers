use egui::Shape;
#[cfg(feature = "vector")]
use egui::{Rect, emath::TSTransform};
#[cfg(feature = "vector")]
use std::sync::Arc;

use crate::ZOrder;
#[cfg(feature = "vector")]
use crate::{Drawable, Text};

/// Everything the map draws in a frame, with labels on top.
pub struct Canvas {
    /// In the order they are drawn.
    shapes: Vec<Shape>,

    #[cfg(feature = "vector")]
    pub(crate) texts: Vec<(ZOrder, Text)>,

    /// Above every map layer, for what is added on top of them.
    pub(crate) z_order: ZOrder,

    /// Which frame this is, for the renderer to tell which drawables are still in use.
    #[cfg(feature = "vector")]
    frame: u64,

    #[cfg(feature = "vector")]
    screen: Rect,
}

/// Drawables to be put on the [`Canvas`], and how. Cheap to clone, and drawing the same ones
/// again does not upload them to the GPU again.
#[cfg(feature = "vector")]
#[derive(Clone)]
pub struct Drawables {
    /// Behind an `Arc`, because that is what the renderer tells uploaded drawables by.
    drawables: Arc<Vec<Drawable>>,
    transform: TSTransform,
    clip_rect: Rect,
    transparency: f32,
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
    pub(crate) fn new(ctx: &egui::Context) -> Self {
        #[cfg(not(feature = "vector"))]
        let _ = ctx;

        Self {
            shapes: Vec::new(),
            #[cfg(feature = "vector")]
            texts: Vec::new(),
            z_order: ZOrder::default(),
            #[cfg(feature = "vector")]
            frame: ctx.cumulative_pass_nr(),
            #[cfg(feature = "vector")]
            screen: ctx.viewport_rect(),
        }
    }

    /// Seed for what is put here from [`crate::Map::show`]'s closure.
    pub fn z_order(&self) -> ZOrder {
        self.z_order
    }

    /// `text` is in screen coordinates.
    #[cfg(feature = "vector")]
    pub fn add_text(&mut self, z_order: ZOrder, text: Text) {
        self.texts.push((z_order, text));
    }

    #[cfg(feature = "vector")]
    pub fn add_drawables(&mut self, drawables: Drawables) {
        let Drawables {
            drawables,
            transform,
            clip_rect,
            transparency,
        } = drawables;

        self.shapes.extend((0..drawables.len()).map(|index| {
            crate::egui_backend::wgpu::Run::callback(
                drawables.to_owned(),
                index,
                transform,
                self.screen,
                clip_rect,
                self.frame,
                transparency,
            )
        }));
    }

    pub(crate) fn add_shape(&mut self, shape: Shape) {
        self.shapes.push(shape);
    }

    /// Draw everything into `slot`, which `painter` reserved earlier, so that it goes below
    /// what was painted since.
    #[cfg_attr(not(feature = "vector"), allow(unused_mut))]
    pub(crate) fn draw(self, painter: &egui::Painter, slot: egui::layers::ShapeIdx) {
        let mut shapes = self.shapes;

        // Stable, so that within a style layer the tiles nearest the center still go first.
        #[cfg(feature = "vector")]
        {
            let mut texts = self.texts;
            texts.sort_by_key(|(z_order, _)| *z_order);
            let texts = texts.into_iter().map(|(_, text)| text).collect();
            shapes.extend(crate::text::place_texts(texts, painter.ctx()));
        }

        painter.set(slot, shapes);
    }
}
