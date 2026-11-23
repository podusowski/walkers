//! Small apps showing one feature of Walkers each.

mod raster;
mod waymark;

use eframe::{App, CreationContext};

type New = fn(&CreationContext<'_>) -> Box<dyn App>;

/// Name and constructor of every demo.
pub const DEMOS: &[(&str, New)] = &[
    ("raster", |cc| Box::new(raster::RasterMap::new(cc))),
    ("waymark", |cc| Box::new(waymark::WaymarkMap::new(cc))),
];

/// The demo called `name`, or `None` if there is no such demo.
pub fn app(name: &str, cc: &CreationContext<'_>) -> Option<Box<dyn App>> {
    DEMOS
        .iter()
        .find(|(demo, _)| *demo == name)
        .map(|(_, new)| new(cc))
}

/// Names of all demos, separated by commas.
pub fn names() -> String {
    DEMOS
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join(", ")
}
