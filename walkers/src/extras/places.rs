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
    fn draw<T: Place>(places: Vec<T>, position: Position, projector: &Projector, ui: &Ui);
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
        // TODO: Implement grouping logic

        let position = center(
            &self
                .places
                .iter()
                .map(|place| place.position())
                .collect::<Vec<_>>(),
        );
        T::Group::draw(self.places, position, projector, ui);
    }
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
