use eframe::{NativeOptions, Renderer};
use winit::platform::android::activity::AndroidApp;
use winit::platform::android::EventLoopBuilderExtAndroid;

#[no_mangle]
fn android_main(app: AndroidApp) -> Result<(), Box<dyn std::error::Error>> {
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("widnet")
            .with_max_level(log::LevelFilter::Info),
    );
    let mut options = NativeOptions::default();
    //options.renderer = Renderer::Wgpu;
    options.event_loop_builder = Some(Box::new(move |builder| {
        builder.with_android_app(app);
    }));
    eframe::run_native(
        "Widawa Tactical Network",
        options,
        Box::new(|cc| Box::new(demo::MyApp::new(cc.egui_ctx.clone()))),
    )?;

    Ok(())
}
