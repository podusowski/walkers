use demo::Provider;
use walkers::sources::{Attribution, TileSource};
use walkers::TileId;

#[derive(Default)]
pub struct MySource {
    pub name: String,
}

impl MySource {
    pub fn new() -> Self {
        Self {
            name: "Humanitarian OpenStreetMap".to_owned(),
        }
    }
    pub fn get_provider_name(&self) -> Provider {
        Provider::Custom(self.name.clone())
    }
}

impl TileSource for MySource {
    fn tile_url(&self, tile_id: TileId) -> String {
        format!(
            "https://tile-b.openstreetmap.fr/hot/{}/{}/{}.png",
            tile_id.zoom, tile_id.x, tile_id.y
        )
    }

    fn attribution(&self) -> Attribution {
        Attribution {
            text: "Humanitarian OpenStreetMap contributors",
            url: "https://www.openstreetmap.org/copyright",
            logo_light: None,
            logo_dark: None,
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use walkers::Tiles;
    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let source = MySource::new();
        eframe::WebRunner::new()
            .start(
                "the_canvas_id", // hardcode it
                web_options,
                Box::new(|cc| {
                    Box::new(demo::MyApp::new(cc.egui_ctx.clone()).with_provider(
                        source.get_provider_name(),
                        Box::new(Tiles::new(source, cc.egui_ctx.to_owned())),
                    ))
                }),
            )
            .await
            .expect("failed to start eframe");
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    println!("This demo is only meant to be compiled for WASM.");
}
