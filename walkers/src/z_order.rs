use std::cmp::Reverse;

/// Which label is placed first, and so wins where labels overlap. Seeded with the map layer, it
/// is then combined with the layer within it, e.g. a style layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct ZOrder {
    /// Lower first, so the map's own tiles win over the layers drawn above them.
    map_layer: usize,

    /// Higher first, so the top style layers win.
    layer: Reverse<usize>,
}

impl ZOrder {
    pub fn new(map_layer: usize) -> Self {
        Self {
            map_layer,
            layer: Reverse(0),
        }
    }

    pub fn combine(self, layer: usize) -> Self {
        Self {
            layer: Reverse(layer),
            ..self
        }
    }
}
