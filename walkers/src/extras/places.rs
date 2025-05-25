use crate::{Plugin, Projector};
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
    fn draw(&self, ui: &Ui, projector: &crate::Projector);
}

pub trait GroupedPlace {
    type Group: Group;
}

pub trait Group {
    fn draw<T: Place>(places: Vec<T>, ui: &Ui, projector: &Projector);
}

pub struct GroupedPlaces<T>
where
    T: Place,
{
    places: Vec<T>,
}

impl<T> Plugin for GroupedPlaces<T>
where
    T: Place + GroupedPlace,
{
    fn run(self: Box<Self>, ui: &mut Ui, _response: &Response, projector: &crate::Projector) {
        // TODO: Implement grouping logic
        T::Group::draw(self.places, ui, projector);
    }
}
