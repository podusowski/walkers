use std::collections::{hash_map::Entry, HashMap};

use egui::{Mesh, Painter, Pos2, Response, Sense, Ui, Widget};

use crate::{
    mercator::{screen_to_position, PositionExt, TileId},
    Position, Tiles, Zoom,
};

/// The actual map widget. Instances are to be created on each frame, as all necessary state is
/// stored in [`Tiles`] and [`MapMemory`].
///
/// # Examples
///
/// ```
/// # use walkers::{Map, Tiles, MapMemory, Position};
///
/// fn update(ui: &mut egui::Ui, tiles: &mut Tiles, map_memory: &mut MapMemory) {
///     ui.add(Map::new(
///         Some(tiles), // `None`, if you don't want to show any tiles.
///         map_memory,
///         Position::new(17.03664, 51.09916)
///     ));
/// }
/// ```
pub struct Map<'a, 'b> {
    tiles: Option<&'b mut Tiles>,
    memory: &'a mut MapMemory,
    my_position: Position,
}

impl<'a, 'b> Map<'a, 'b> {
    pub fn new(
        tiles: Option<&'b mut Tiles>,
        memory: &'a mut MapMemory,
        my_position: Position,
    ) -> Self {
        Self {
            tiles,
            memory,
            my_position,
        }
    }
}

impl Widget for Map<'_, '_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::drag());

        if response.hovered() {
            let zoom_delta = ui.input(|input| input.zoom_delta());

            // Zooming and dragging need to be exclusive, otherwise the map will get dragged when
            // pinch gesture is used.
            if !(0.99..=1.01).contains(&zoom_delta) {
                // Shift by 1 because of the values given by zoom_delta(). Multiple by 2, because
                // then it felt right with both mouse wheel, and an Android phone.
                self.memory.zoom.zoom_by((zoom_delta - 1.) * 2.);
            } else {
                self.memory
                    .center_mode
                    .drag(&response, self.my_position, self.memory.zoom.round());
            }
        }

        let map_center = self.memory.center_mode.position(self.my_position);
        let painter = ui.painter().with_clip_rect(rect);

        if let Some(tiles) = self.tiles {
            let mut meshes = Default::default();
            draw_tiles(
                &painter,
                map_center.tile_id(self.memory.zoom.round()),
                map_center.project(self.memory.zoom.round()),
                tiles,
                ui,
                &mut meshes,
            );

            for (_, shape) in meshes {
                painter.add(shape);
            }
        }

        response
    }
}

/// Position at the map's center. Initially, the map follows `my_position` argument which typically
/// is meant to be fed by a GPS sensor or other geo-localization method. If user drags the map,
/// it becomes "detached" and stays this way until [`MapMemory::center_mode`] is changed back to
/// [`Center::MyPosition`].
#[derive(Clone, PartialEq)]
pub enum Center {
    /// Center at `my_position` argument of the [`Map::new()`] function.
    MyPosition,

    /// Center at the exact position.
    Exact(Position),
}

impl Center {
    fn drag(&mut self, response: &Response, my_position: Position, zoom: u8) {
        if response.dragged_by(egui::PointerButton::Primary) {
            // We always end up in some exact, "detached" position, regardless of the current mode.
            *self = Center::Exact(screen_to_position(
                self.position(my_position).project(zoom) - response.drag_delta(),
                zoom,
            ));
        }
    }

    /// Get the real position at the map's center.
    pub fn position(&self, my_position: Position) -> Position {
        match self {
            Center::MyPosition => my_position,
            Center::Exact(position) => *position,
        }
    }
}

/// State of the map widget which must persist between frames.
pub struct MapMemory {
    pub center_mode: Center,
    pub zoom: Zoom,
}

impl Default for MapMemory {
    fn default() -> Self {
        Self {
            center_mode: Center::MyPosition,
            zoom: Default::default(),
        }
    }
}

fn draw_tiles(
    painter: &Painter,
    tile_id: TileId,
    map_center_projected_position: Pos2,
    tiles: &mut Tiles,
    ui: &mut Ui,
    meshes: &mut HashMap<TileId, Mesh>,
) {
    let tile_projected = tile_id.project();
    let tile_screen_position = painter.clip_rect().center().to_vec2() + tile_projected.to_vec2()
        - map_center_projected_position.to_vec2();

    let Some(image) = tiles.at(tile_id) else {
        return;
    };

    if painter
        .clip_rect()
        .intersects(image.rect(tile_screen_position))
    {
        if let Entry::Vacant(vacant) = meshes.entry(tile_id) {
            vacant.insert(image.mesh(tile_screen_position, ui.ctx()));

            for coordinates in [
                tile_id.north(),
                tile_id.east(),
                tile_id.south(),
                tile_id.west(),
            ]
            .iter()
            .flatten()
            {
                draw_tiles(
                    painter,
                    *coordinates,
                    map_center_projected_position,
                    tiles,
                    ui,
                    meshes,
                );
            }
        }
    }
}
