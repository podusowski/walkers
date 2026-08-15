mod app;
mod journal;
mod style;

use app::Wanderers;

fn main() -> Result<(), eframe::Error> {
    env_logger::init();

    let path = match std::env::args_os().nth(1).map(Into::into) {
        Some(path) => path,
        None => match journal::default_path() {
            Some(path) => path,
            None => {
                eprintln!("Could not tell where to keep the journal, pass a path explicitly.");
                std::process::exit(1);
            }
        },
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Wanderers")
            .with_inner_size([1024., 768.]),
        ..Default::default()
    };

    eframe::run_native(
        "wanderers",
        options,
        Box::new(|cc| {
            style::apply(&cc.egui_ctx);
            Ok(Box::new(Wanderers::new(cc.egui_ctx.to_owned(), path)))
        }),
    )
}
