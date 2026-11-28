//! Runs the demo named by the first argument natively, or by the URL's hash in the browser.

mod features;
mod raster;
mod waymark;

use eframe::{App, CreationContext};

type New = fn(&CreationContext<'_>) -> Box<dyn App>;

/// Name and constructor of every demo.
const DEMOS: &[(&str, New)] = &[
    ("raster", |cc| Box::new(raster::RasterMap::new(cc))),
    ("waymark", |cc| Box::new(waymark::WaymarkMap::new(cc))),
    ("features", |cc| Box::new(features::FeaturesMap::new(cc))),
];

/// The demo called `name`, or `None` if there is no such demo.
fn app(name: &str, cc: &CreationContext<'_>) -> Option<Box<dyn App>> {
    DEMOS
        .iter()
        .find(|(demo, _)| *demo == name)
        .map(|(_, new)| new(cc))
}

/// Names of all demos, separated by commas.
fn names() -> String {
    DEMOS
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), eframe::Error> {
    env_logger::init();

    let Some(name) = std::env::args().nth(1) else {
        eprintln!("Usage: cargo run -p demos -- <demo>, one of: {}.", names());
        std::process::exit(2);
    };

    // ANCHOR: renderer
    eframe::run_native(
        &format!("Walkers: {name}"),
        Default::default(),
        Box::new(move |cc| {
            walkers::install_renderer(cc.wgpu_render_state.as_ref());
            Ok(app(&name, cc).unwrap_or_else(|| Box::new(Unknown(name))))
        }),
    )
    // ANCHOR_END: renderer
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast;

    eframe::WebLogger::init(log::LevelFilter::Info).ok();

    wasm_bindgen_futures::spawn_local(async {
        let canvas = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id("the_canvas_id"))
            .and_then(|canvas| canvas.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            .expect("there is no canvas to draw on");

        eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| {
                    walkers::install_renderer(cc.wgpu_render_state.as_ref());
                    let hash = &cc.integration_info.web_info.location.hash;
                    let name = hash.trim_start_matches('#').to_owned();
                    Ok(app(&name, cc).unwrap_or_else(|| Box::new(Unknown(name))))
                }),
            )
            .await
            .expect("failed to start eframe");
    });
}

/// Says that there is no demo of that name.
struct Unknown(String);

impl App for Unknown {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.label(format!(
            "There is no demo called \"{}\". Try one of: {}.",
            self.0,
            names()
        ));
    }
}
