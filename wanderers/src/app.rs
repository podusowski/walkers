use std::path::PathBuf;

use egui::{Align2, Image, RichText, Ui, Window};
use walkers::{HttpOptions, HttpTiles, Map, MapMemory, Position, Tiles, lon_lat, sources};

/// Where the map is centered when the app starts, until the journal has a say in it.
fn home() -> Position {
    lon_lat(17.032094, 51.110090)
}

/// Tiles are cached between runs, so that the tile server is not hit more than needed.
fn http_options() -> HttpOptions {
    HttpOptions {
        cache: cache_dir(),
        ..Default::default()
    }
}

fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::home_dir().map(|home| home.join(".cache")))?;
    Some(base.join("wanderers").join("tiles"))
}

pub struct Wanderers {
    tiles: HttpTiles,
    map_memory: MapMemory,
}

impl Wanderers {
    pub fn new(egui_ctx: egui::Context) -> Self {
        Self {
            tiles: HttpTiles::with_options(sources::OpenStreetMap, http_options(), egui_ctx),
            map_memory: MapMemory::default(),
        }
    }
}

impl eframe::App for Wanderers {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let attribution = self.tiles.attribution();

        Map::new(Some(&mut self.tiles), &mut self.map_memory, home())
            .zoom_with_ctrl(false)
            .show(ui, |_, _, _, _| {});

        zoom(ui, &mut self.map_memory);
        go_home(ui, &mut self.map_memory);
        acknowledge(ui, attribution);
    }
}

/// Buttons to zoom in and out, for when there is no scroll wheel around.
fn zoom(ui: &Ui, map_memory: &mut MapMemory) {
    Window::new("Zoom")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::LEFT_BOTTOM, [10., -10.])
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                if ui.button(RichText::new("➕").heading()).clicked() {
                    let _ = map_memory.zoom_in();
                }

                if ui.button(RichText::new("➖").heading()).clicked() {
                    let _ = map_memory.zoom_out();
                }
            });
        });
}

/// Once the map is dragged away, offer a way back.
fn go_home(ui: &Ui, map_memory: &mut MapMemory) {
    if map_memory.detached().is_some() {
        Window::new("Go home")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(Align2::RIGHT_BOTTOM, [-10., -10.])
            .show(ui.ctx(), |ui| {
                if ui.button(RichText::new("go back home").heading()).clicked() {
                    map_memory.follow_my_position();
                }
            });
    }
}

/// OpenStreetMap requires the map to be credited, wherever it is shown.
fn acknowledge(ui: &Ui, attribution: sources::Attribution) {
    Window::new("Acknowledge")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::LEFT_TOP, [10., 10.])
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                if let Some(logo) = attribution.logo_light {
                    ui.add(Image::new(logo).max_height(30.).max_width(80.));
                }
                ui.hyperlink_to(attribution.text, attribution.url);
            });
        });
}
