mod app;

use app::MyApp;
use egui::{Align2, Context, Painter, Shape};
use walkers::{Map, MapMemory, Projector, Tiles};

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    eframe::run_native(
        "MyApp",
        Default::default(),
        Box::new(|cc| Box::new(MyApp::new(cc.egui_ctx.clone()))),
    )
}
