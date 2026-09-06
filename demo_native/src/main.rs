#[cfg(not(target_arch = "wasm32"))]
use demo::MyApp;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    eframe::run_native(
        "MyApp",
        Default::default(),
        Box::new(|cc| {
            walkers::install_renderer(cc.wgpu_render_state.as_ref());
            Ok(Box::new(MyApp::new(cc)))
        }),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    println!("This demo is not meant to be compiled for WASM.");
}
