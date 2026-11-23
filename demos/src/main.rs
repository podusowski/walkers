//! Runs the demo named by the first argument natively, or by the URL's hash in the browser.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), eframe::Error> {
    env_logger::init();

    let Some(name) = std::env::args().nth(1) else {
        eprintln!(
            "Usage: cargo run -p demos -- <demo>, one of: {}.",
            demos::names()
        );
        std::process::exit(2);
    };

    // ANCHOR: renderer
    eframe::run_native(
        &format!("Walkers: {name}"),
        Default::default(),
        Box::new(move |cc| {
            walkers::install_renderer(cc.wgpu_render_state.as_ref());
            Ok(demos::app(&name, cc).unwrap_or_else(|| Box::new(Unknown(name))))
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
                    Ok(demos::app(&name, cc).unwrap_or_else(|| Box::new(Unknown(name))))
                }),
            )
            .await
            .expect("failed to start eframe");
    });
}

/// Says that there is no demo of that name.
struct Unknown(String);

impl eframe::App for Unknown {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.label(format!(
            "There is no demo called \"{}\". Try one of: {}.",
            self.0,
            demos::names()
        ));
    }
}
