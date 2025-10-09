use crate::{MapMemory, Plugin, Position, Projector};
use egui::{Id, Rect, Response, Sense, Ui, vec2};

/// [`Plugin`] which shows places on the map. Place can be any type that implements the [`Place`]
/// trait.
pub struct Places<T>
where
    T: Place,
{
    places: Vec<T>,
}

impl<T> Places<T>
where
    T: Place,
{
    pub fn new(places: Vec<T>) -> Self {
        Self { places }
    }
}

impl<T> Plugin for Places<T>
where
    T: Place + 'static,
{
    fn run(
        self: Box<Self>,
        ui: &mut Ui,
        _response: &Response,
        projector: &Projector,
        _map_memory: &MapMemory,
    ) {
        for place in &self.places {
            place.draw(ui, projector);
        }
    }
}

pub trait Place {
    fn position(&self) -> Position;
    fn draw(&self, ui: &Ui, projector: &Projector);
}

/// A group of places that can be drawn together on the map.
pub trait Group {
    fn draw<T: Place>(&self, places: &[&T], position: Position, projector: &Projector, ui: &mut Ui);
}

/// Similar to [`Places`], but groups places that are close together and draws them as a
/// single [`Group`].
pub struct GroupedPlaces<T, G>
where
    T: Place,
    G: Group,
{
    places: Vec<T>,
    group: G,
}

impl<T, G> GroupedPlaces<T, G>
where
    T: Place,
    G: Group,
{
    pub fn new(places: Vec<T>, group: G) -> Self {
        Self { places, group }
    }

    /// Handle user interactions. Returns whether group should be expanded.
    fn interact(&self, position: Position, projector: &Projector, ui: &Ui, id: Id) -> bool {
        let screen_position = projector.project(position);
        let rect = Rect::from_center_size(screen_position.to_pos2(), vec2(50., 50.));
        let response = ui.interact(rect, id, Sense::click());

        if response.clicked() {
            // Toggle the visibility of the group when clicked.
            let expand = ui.ctx().memory_mut(|m| {
                let expand = m.data.get_temp::<bool>(id).unwrap_or(false);
                m.data.insert_temp(id, !expand);
                expand
            });
            expand
        } else {
            ui.ctx()
                .memory(|m| m.data.get_temp::<bool>(id).unwrap_or(false))
        }
    }
}

impl<T, G> Plugin for GroupedPlaces<T, G>
where
    T: Place,
    G: Group,
{
    fn run(
        self: Box<Self>,
        ui: &mut Ui,
        _response: &Response,
        projector: &Projector,
        _map_memory: &MapMemory,
    ) {
        for (idx, places) in groups(&self.places, projector).iter().enumerate() {
            let id = ui.id().with(idx);
            let position = center(&places.iter().map(|p| p.position()).collect::<Vec<_>>());
            let expand = self.interact(position, projector, ui, id);

            if places.len() >= 2 && !expand {
                self.group.draw(places, position, projector, ui);
            } else {
                for place in places {
                    place.draw(ui, projector);
                }
            }
        }
    }
}

/// Group places that are close together.
fn groups<'a, T>(places: &'a [T], projector: &Projector) -> Vec<Vec<&'a T>>
where
    T: Place,
{
    let mut groups: Vec<Vec<&T>> = Vec::new();

    for place in places {
        if let Some(group) = groups.iter_mut().find(|g| {
            g.iter()
                .all(|p| distance_projected(place.position(), p.position(), projector) < 50.0)
        }) {
            group.push(place);
        } else {
            groups.push(vec![place]);
        }
    }

    groups
}

/// Calculate the distance between two positions after being projected onto the screen.
fn distance_projected(p1: Position, p2: Position, projector: &Projector) -> f32 {
    let screen_p1 = projector.project(p1).to_pos2();
    let screen_p2 = projector.project(p2).to_pos2();
    (screen_p1 - screen_p2).length()
}

fn center(positions: &[Position]) -> Position {
    if positions.is_empty() {
        Position::default()
    } else {
        let sum = positions
            .iter()
            .fold(Position::default(), |acc, &p| acc + p);
        sum / positions.len() as f64
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn calculating_center() {
        use super::*;

        assert_eq!(
            center(&[
                Position::new(0.0, 0.0),
                Position::new(10.0, 10.0),
                Position::new(20.0, 20.0),
            ]),
            Position::new(10.0, 10.0)
        );

        assert_eq!(
            center(&[
                Position::new(0.0, 0.0),
                Position::new(10.0, 0.0),
                Position::new(0.0, 10.0),
                Position::new(10.0, 10.0),
            ]),
            Position::new(5.0, 5.0)
        );

        assert_eq!(
            center(&[
                Position::new(10.0, 10.0),
                Position::new(-10.0, -10.0),
                Position::new(-10.0, 10.0),
                Position::new(10.0, -10.0),
            ]),
            Position::new(0.0, 0.0)
        );
    }
}
