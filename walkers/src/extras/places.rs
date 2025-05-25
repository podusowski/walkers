use crate::{Plugin, Position, Projector};
use egui::{Response, Ui};

/// [`Plugin`] which draws list of places on the map.
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
    fn run(self: Box<Self>, ui: &mut Ui, _response: &Response, projector: &crate::Projector) {
        for place in &self.places {
            place.draw(ui, projector);
        }
    }
}

pub trait Place {
    fn position(&self) -> Position;
    fn draw(&self, ui: &Ui, projector: &crate::Projector);
}

pub trait GroupedPlace {
    type Group: Group;
}

/// Trait that can be implemented by a [`Place`] to provide grouping functionality.
pub trait Group {
    fn draw<T: Place>(places: &[&T], position: Position, projector: &Projector, ui: &Ui);
}

pub struct GroupedPlaces<T>
where
    T: Place,
{
    places: Vec<T>,
}

impl<T> GroupedPlaces<T>
where
    T: Place + GroupedPlace,
{
    pub fn new(places: Vec<T>) -> Self {
        Self { places }
    }
}

impl<T> Plugin for GroupedPlaces<T>
where
    T: Place + GroupedPlace,
{
    fn run(self: Box<Self>, ui: &mut Ui, _response: &Response, projector: &crate::Projector) {
        let mut groups: Vec<Vec<&T>> = Vec::new();

        for place in &self.places {
            if let Some(group) = groups.iter_mut().find(|g| {
                g.iter()
                    .all(|p| near_on_screen(place.position(), p.position(), projector))
            }) {
                group.push(place);
            } else {
                groups.push(vec![place]);
            }
        }

        for group in groups {
            if group.len() >= 2 {
                T::Group::draw(
                    &group,
                    center(&group.iter().map(|p| p.position()).collect::<Vec<_>>()),
                    projector,
                    ui,
                );
            } else {
                for place in group {
                    place.draw(ui, projector);
                }
            }
        }
    }
}

fn near_on_screen(p1: Position, p2: Position, projector: &Projector) -> bool {
    let screen_p1 = projector.project(p1).to_pos2();
    let screen_p2 = projector.project(p2).to_pos2();
    (screen_p1 - screen_p2).length() < 50.0
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
