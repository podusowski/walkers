use egui::Shape;
#[cfg(feature = "mvt")]
use egui::{Rect, emath::TSTransform};
#[cfg(feature = "mvt")]
use std::sync::Arc;

use crate::ZOrder;
#[cfg(feature = "mvt")]
use crate::{Drawable, Text};

/// Everything the map draws in a frame: the tiles, and what [`crate::Map::show`]'s closure puts
/// here. It goes below anything the closure paints with egui, and the labels are placed
/// together, above everything else on the canvas.
#[derive(Default)]
pub struct Canvas {
    /// In screen coordinates.
    #[cfg(feature = "mvt")]
    pub drawables: Vec<Drawable>,

    #[cfg(feature = "mvt")]
    pub(crate) texts: Vec<(ZOrder, Text)>,

    /// Tiles of every map layer, in the order they are drawn.
    pieces: Vec<Piece>,

    /// For what the closure puts here, which comes above every map layer.
    pub(crate) z_order: ZOrder,
}

enum Piece {
    Shape(Shape),

    #[cfg(feature = "mvt")]
    Drawables {
        /// Behind an `Arc`, because that is what the renderer tells uploaded drawables by.
        drawables: Arc<Vec<Drawable>>,
        transform: TSTransform,
        clip_rect: Rect,
        transparency: f32,
    },
}

impl Canvas {
    /// Seed for what is put here from [`crate::Map::show`]'s closure.
    pub fn z_order(&self) -> ZOrder {
        self.z_order
    }

    /// `text` is in screen coordinates.
    #[cfg(feature = "mvt")]
    pub fn add_text(&mut self, z_order: ZOrder, text: Text) {
        self.texts.push((z_order, text));
    }

    pub(crate) fn add_shape(&mut self, shape: Shape) {
        self.pieces.push(Piece::Shape(shape));
    }

    #[cfg(feature = "mvt")]
    pub(crate) fn add_drawables(
        &mut self,
        drawables: Arc<Vec<Drawable>>,
        transform: TSTransform,
        clip_rect: Rect,
        transparency: f32,
    ) {
        self.pieces.push(Piece::Drawables {
            drawables,
            transform,
            clip_rect,
            transparency,
        });
    }

    /// Draw everything into `slot`, which `painter` reserved earlier, so that it goes below
    /// what was painted since.
    #[cfg_attr(not(feature = "mvt"), allow(unused_mut))]
    pub(crate) fn draw(mut self, painter: &egui::Painter, slot: egui::layers::ShapeIdx) {
        let ctx = painter.ctx();

        #[cfg(feature = "mvt")]
        {
            let drawables = std::mem::take(&mut self.drawables);
            self.add_drawables(
                Arc::new(crate::render::merge_runs(drawables)),
                TSTransform::IDENTITY,
                painter.clip_rect(),
                1.0,
            );
        }

        #[cfg(feature = "mvt")]
        let (frame, screen) = (ctx.cumulative_pass_nr(), ctx.viewport_rect());

        let mut shapes: Vec<Shape> = self
            .pieces
            .into_iter()
            .flat_map(|piece| match piece {
                Piece::Shape(shape) => vec![shape],
                #[cfg(feature = "mvt")]
                Piece::Drawables {
                    drawables,
                    transform,
                    clip_rect,
                    transparency,
                } => (0..drawables.len())
                    .map(|index| {
                        crate::egui_backend::wgpu::Run::callback(
                            drawables.to_owned(),
                            index,
                            transform,
                            screen,
                            clip_rect,
                            frame,
                            transparency,
                        )
                    })
                    .collect(),
            })
            .collect();

        // Stable, so that within a style layer the tiles nearest the center still go first.
        #[cfg(feature = "mvt")]
        {
            let mut texts = self.texts;
            texts.sort_by_key(|(z_order, _)| *z_order);
            let texts = texts.into_iter().map(|(_, text)| text).collect();
            shapes.extend(crate::text::place_texts(texts, ctx));
        }
        #[cfg(not(feature = "mvt"))]
        let _ = ctx;

        painter.set(slot, shapes);
    }
}
