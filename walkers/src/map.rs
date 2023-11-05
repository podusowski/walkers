use std::collections::{hash_map::Entry, HashMap};

use egui::{Context, Mesh, Painter, Pos2, Rect, Response, Sense, Ui, Vec2, Widget};

use crate::{
    mercator::{screen_to_position, PositionExt, TileId},
    tiles,
    zoom::{InvalidZoom, Zoom},
    Position, Tiles,
};

/// Plugins allow drawing custom shapes on the map. After implementing this trait for your type,
/// you can add it to the map with [`Map::with_plugin`]
pub trait Plugin {
    /// Function called at each frame.
    fn draw(&self, painter: Painter, projector: &Projector);
}

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
    plugins: Vec<Box<dyn Plugin>>,
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
            plugins: Vec::default(),
        }
    }

    /// Add plugin to the drawing pipeline. Plugins allow drawing custom shaped on the map.
    pub fn with_plugin(mut self, plugin: impl Plugin + 'static) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }
}

/// Projects geographical position into screen pixels, suitable for [`egui::Painter`].
#[derive(Clone)]
pub struct Projector {
    clip_rect: Rect,
    memory: MapMemory,
    my_position: Position,
}

impl Projector {
    pub fn project(&self, position: Position) -> Vec2 {
        // Turn that into a flat, mercator projection.
        let projected_position = position.project(self.memory.zoom.round());

        let zoom = self.memory.zoom.round();

        // We also need to know where the map center is.
        let map_center_projected_position = self
            .memory
            .center_mode
            .position(self.my_position, zoom)
            .project(self.memory.zoom.round());

        // From the two points above we can calculate the actual point on the screen.
        self.clip_rect.center() + projected_position.to_vec2() - map_center_projected_position
    }
}

impl Map<'_, '_> {
    /// Handle zoom and drag inputs, and recalculate everything accordingly.
    fn zoom_and_drag(&mut self, ui: &mut Ui, response: &Response) {
        let zoom_delta = ui.input(|input| input.zoom_delta());

        // Zooming and dragging need to be exclusive, otherwise the map will get dragged when
        // pinch gesture is used.
        if !(0.99..=1.01).contains(&zoom_delta) {
            // Shift by 1 because of the values given by zoom_delta(). Multiple by 2, because
            // then it felt right with both mouse wheel, and an Android phone.
            self.memory.zoom.zoom_by((zoom_delta - 1.) * 2.);

            // Recalculate the AdjustedPosition's offset, since it gets invalidated by zooming.
            self.memory.center_mode = self
                .memory
                .center_mode
                .clone()
                .zero_offset(self.memory.zoom.round());
        } else {
            self.memory
                .center_mode
                .recalculate_drag(&response, self.my_position);
        }

        self.memory
            .center_mode
            .recalculate_inertial_movement(ui.ctx());
    }
}

impl Widget for Map<'_, '_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::drag());

        self.zoom_and_drag(ui, &response);

        let zoom = self.memory.zoom.round();
        let map_center = self.memory.center_mode.position(self.my_position, zoom);
        let painter = ui.painter().with_clip_rect(rect);

        if let Some(tiles) = self.tiles {
            let mut meshes = Default::default();
            flood_fill_tiles(
                &painter,
                map_center.tile_id(zoom),
                map_center.project(zoom),
                tiles,
                ui,
                &mut meshes,
            );

            for shape in meshes.drain().filter_map(|(_, mesh)| mesh) {
                painter.add(shape);
            }
        }

        for plugin in self.plugins {
            let projector = Projector {
                clip_rect: response.rect,
                memory: self.memory.to_owned(),
                my_position: self.my_position,
            };

            plugin.draw(painter.to_owned(), &projector);
        }

        response
    }
}

/// [`Position`] alone is not able to represent detached (e.g. after map gets dragged) position
/// due to insufficient accuracy.
#[derive(Debug, Clone, PartialEq)]
pub struct AdjustedPosition {
    /// Base geographical position.
    position: Position,

    /// Offset in pixels.
    offset: Vec2,
}

impl AdjustedPosition {
    /// Calculate the real position, i.e. including the offset.
    fn position(&self, zoom: u8) -> Position {
        screen_to_position(self.position.project(zoom) - self.offset, zoom)
    }

    /// Recalculate `position` so that `offset` is zero.
    fn zero_offset(self, zoom: u8) -> Self {
        Self {
            position: screen_to_position(self.position.project(zoom) - self.offset, zoom),
            offset: Vec2::ZERO,
        }
    }
}

/// Position at the map's center. Initially, the map follows `my_position` argument which typically
/// is meant to be fed by a GPS sensor or other geo-localization method. If user drags the map,
/// it becomes "detached" and stays this way until [`MapMemory::center_mode`] is changed back to
/// [`Center::MyPosition`].
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Center {
    /// Centered at `my_position` argument of the [`Map::new()`] function.
    #[default]
    MyPosition,

    /// Centered at the exact position.
    Exact(AdjustedPosition),

    /// Map's currently moving due to inertia, and will slow down and stop after a short while.
    Inertia {
        position: AdjustedPosition,
        direction: Vec2,
        amount: f32,
    },
}

impl Center {
    fn recalculate_drag(&mut self, response: &Response, my_position: Position) {
        if response.dragged_by(egui::PointerButton::Primary) {
            let position = match &self {
                Center::MyPosition => AdjustedPosition {
                    position: my_position,
                    offset: Vec2::ZERO,
                },
                Center::Exact(position) | Center::Inertia { position, .. } => position.to_owned(),
            };

            *self = Center::Inertia {
                position,
                direction: response.drag_delta(),
                amount: 1.0,
            };
        }
    }

    fn recalculate_inertial_movement(&mut self, ctx: &Context) {
        if let Center::Inertia {
            position,
            direction,
            amount,
        } = &self
        {
            *self = if amount <= &mut 0.0 {
                Center::Exact(position.to_owned())
            } else {
                let offset = position.offset + (*direction * *amount);

                Center::Inertia {
                    position: AdjustedPosition {
                        position: position.position,
                        offset,
                    },
                    direction: *direction,
                    amount: *amount - 0.03,
                }
            };

            // Map is moving due to interia, therefore we need to recalculate in the next frame.
            log::trace!("Requesting repaint due to non-zero inertia.");
            ctx.request_repaint();
        }
    }

    /// Returns exact position if map is detached (i.e. not following `my_position`),
    /// `None` otherwise.
    fn detached(&self, zoom: u8) -> Option<Position> {
        match self {
            Center::MyPosition => None,
            Center::Exact(position) | Center::Inertia { position, .. } => {
                Some(position.position(zoom))
            }
        }
    }

    /// Get the real position at the map's center.
    pub fn position(&self, my_position: Position, zoom: u8) -> Position {
        self.detached(zoom).unwrap_or(my_position)
    }

    pub fn zero_offset(self, zoom: u8) -> Self {
        match self {
            Center::MyPosition => Center::MyPosition,
            Center::Exact(position) => Center::Exact(position.zero_offset(zoom)),
            Center::Inertia {
                position,
                direction,
                amount,
            } => Center::Inertia {
                position: position.zero_offset(zoom),
                direction,
                amount,
            },
        }
    }
}

/// State of the map widget which must persist between frames.
#[derive(Debug, Default, Clone)]
pub struct MapMemory {
    center_mode: Center,
    zoom: Zoom,
}

impl MapMemory {
    /// Try to zoom in, returning `Err(InvalidZoom)` if already at maximum.
    pub fn zoom_in(&mut self) -> Result<(), InvalidZoom> {
        self.center_mode = self.center_mode.clone().zero_offset(self.zoom.round());
        self.zoom.zoom_in()
    }

    /// Try to zoom out, returning `Err(InvalidZoom)` if already at minimum.
    pub fn zoom_out(&mut self) -> Result<(), InvalidZoom> {
        self.center_mode = self.center_mode.clone().zero_offset(self.zoom.round());
        self.zoom.zoom_out()
    }

    /// Returns exact position if map is detached (i.e. not following `my_position`),
    /// `None` otherwise.
    pub fn detached(&self) -> Option<Position> {
        self.center_mode.detached(self.zoom.round())
    }

    /// Center exactly at the given position.
    pub fn center_at(&mut self, position: Position) {
        self.center_mode = Center::Exact(AdjustedPosition {
            position,
            offset: Vec2::ZERO,
        });
    }

    /// Follow `my_position`.
    pub fn follow_my_position(&mut self) {
        self.center_mode = Center::MyPosition;
    }
}

/// Use simple [flood fill algorithm](https://en.wikipedia.org/wiki/Flood_fill) to draw tiles on the map.
fn flood_fill_tiles(
    painter: &Painter,
    tile_id: TileId,
    map_center_projected_position: Pos2,
    tiles: &mut Tiles,
    ui: &mut Ui,
    meshes: &mut HashMap<TileId, Option<Mesh>>,
) {
    let tile_projected = tile_id.project();
    let tile_screen_position = painter.clip_rect().center().to_vec2() + tile_projected.to_vec2()
        - map_center_projected_position.to_vec2();

    if painter
        .clip_rect()
        .intersects(tiles::rect(tile_screen_position))
    {
        if let Entry::Vacant(entry) = meshes.entry(tile_id) {
            // It's still OK to insert an empty one, as we need to mark the spot for the filling algorithm.
            let tile = tiles
                .at(tile_id)
                .map(|tile| tile.mesh(tile_screen_position, ui.ctx()));

            entry.insert(tile);

            for coordinates in [
                tile_id.north(),
                tile_id.east(),
                tile_id.south(),
                tile_id.west(),
            ]
            .iter()
            .flatten()
            {
                flood_fill_tiles(
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
