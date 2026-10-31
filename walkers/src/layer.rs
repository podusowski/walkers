use crate::{Canvas, Projector, Tiles, tiles::draw_tiles};

/// Something the map draws onto its [`Canvas`], below plugins and [`crate::Map::show`]'s
/// closure. Add it to the map with [`crate::Map::with_layer`].
pub trait Layer {
    fn draw(&mut self, canvas: &mut Canvas, projector: &Projector);
}

impl<T: Tiles + ?Sized> Layer for &mut T {
    fn draw(&mut self, canvas: &mut Canvas, projector: &Projector) {
        draw_tiles(canvas, projector, &mut **self);
    }
}
