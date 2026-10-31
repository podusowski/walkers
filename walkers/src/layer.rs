use crate::{Canvas, Projector, Tiles, tiles::draw_tiles};

pub trait Layer {
    fn draw(&mut self, canvas: &mut Canvas, projector: &Projector);
}

impl<T: Tiles + ?Sized> Layer for &mut T {
    fn draw(&mut self, canvas: &mut Canvas, projector: &Projector) {
        draw_tiles(canvas, projector, &mut **self);
    }
}
