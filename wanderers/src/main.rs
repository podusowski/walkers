mod app;

use app::Wanderers;

fn main() -> Result<(), eframe::Error> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Wanderers")
            .with_inner_size([1024., 768.]),
        ..Default::default()
    };

    eframe::run_native(
        "wanderers",
        options,
        Box::new(|cc| Ok(Box::new(Wanderers::new(cc.egui_ctx.to_owned())))),
    )
}
