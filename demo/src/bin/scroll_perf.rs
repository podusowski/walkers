//! Scrolls the map around on its own and reports how fast it can be drawn.
//!
//! Pass a `.pmtiles` file to use it, otherwise the map comes from OpenFreeMap.
//!
//! ```text
//! cargo run --release -p demo --bin scroll_perf --features mvt,pmtiles [file.pmtiles]
//! ```

use std::collections::VecDeque;
use std::time::Instant;

use egui::{Align2, Key, Window};
use walkers::{HttpOptions, HttpTiles, Map, MapMemory, Position, Style, Tiles, lon_lat, sources};

/// Wrocław, which has enough going on to be worth drawing.
fn start() -> Position {
    lon_lat(17.032094, 51.110090)
}

/// How long the map takes to go once around its circle.
const SECONDS_PER_LAP: f64 = 20.0;

/// Radius of that circle, counted in tiles so that it means the same however far the map is
/// zoomed in. Wide enough to keep reaching tiles which were not on screen a moment ago.
const RADIUS_TILES: f64 = 2.5;

const STARTING_ZOOM: f64 = 15.0;

fn radius_degrees() -> f64 {
    RADIUS_TILES * 360. / 2f64.powf(STARTING_ZOOM)
}

/// Frames the reported average is taken over.
const AVERAGED_FRAMES: usize = 120;

/// Fixed, because how much there is to draw follows the size of the window, and numbers from
/// two runs are only worth comparing if it was the same both times.
const WINDOW_SIZE: [f32; 2] = [1280., 800.];

/// Mean of the last [`AVERAGED_FRAMES`] frames, so the number on screen does not flicker.
struct Rolling {
    samples: VecDeque<f64>,
}

impl Rolling {
    fn new() -> Self {
        Self {
            samples: VecDeque::with_capacity(AVERAGED_FRAMES),
        }
    }

    fn push(&mut self, value: f64) {
        if self.samples.len() == AVERAGED_FRAMES {
            self.samples.pop_front();
        }
        self.samples.push_back(value);
    }

    fn mean(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }
}

struct ScrollPerf {
    tiles: Box<dyn Tiles>,
    source: String,
    memory: MapMemory,

    /// Where on the circle the map is, in laps. Advanced by the time between frames rather
    /// than by a frame count, so the map moves at the same speed however fast it draws.
    lap: f64,

    scrolling: bool,
    previous_frame: Option<Instant>,
    frame_ms: Rolling,
}

impl ScrollPerf {
    fn new(egui_ctx: egui::Context) -> Self {
        let (tiles, source): (Box<dyn Tiles>, String) = match std::env::args().nth(1) {
            Some(path) => (
                Box::new(walkers::PmTiles::with_style(
                    &path,
                    Style::protomaps_basemap_light(),
                    egui_ctx,
                )),
                path,
            ),
            None => (
                Box::new(HttpTiles::with_options_and_style(
                    sources::OpenFreeMap,
                    HttpOptions::default(),
                    Style::openmaptiles_basemap_light(),
                    egui_ctx,
                )),
                "OpenFreeMap".to_owned(),
            ),
        };

        let mut memory = MapMemory::default();
        let _ = memory.set_zoom(STARTING_ZOOM);

        Self {
            tiles,
            source,
            memory,
            lap: 0.0,
            scrolling: true,
            previous_frame: None,
            frame_ms: Rolling::new(),
        }
    }

    /// Where the map should be centered, given how far around the circle it is.
    fn position(&self) -> Position {
        let angle = self.lap * std::f64::consts::TAU;
        let start = start();

        let radius = radius_degrees();

        lon_lat(
            start.x() + radius * angle.cos(),
            // Scaled so that the circle looks like one rather than an ellipse.
            start.y() + radius * angle.sin() * start.y().to_radians().cos(),
        )
    }
}

impl eframe::App for ScrollPerf {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.ctx().request_repaint();

        let now = Instant::now();
        let since_previous = self
            .previous_frame
            .replace(now)
            .map(|previous| now - previous);

        if let Some(elapsed) = since_previous {
            self.frame_ms.push(elapsed.as_secs_f64() * 1000.);

            if self.scrolling {
                self.lap += elapsed.as_secs_f64() / SECONDS_PER_LAP;
                self.memory.center_at(self.position());
            }
        }

        if ui.input(|input| input.key_pressed(Key::Space)) {
            self.scrolling = !self.scrolling;
        }
        if ui.input(|input| input.key_pressed(Key::Plus) || input.key_pressed(Key::Equals)) {
            let _ = self.memory.zoom_in();
        }
        if ui.input(|input| input.key_pressed(Key::Minus)) {
            let _ = self.memory.zoom_out();
        }

        egui::CentralPanel::default().show(ui, |ui| {
            Map::new(Some(self.tiles.as_mut()), &mut self.memory, start())
                .show(ui, |_, _, _, _| {});
        });

        let frame_ms = self.frame_ms.mean();
        let fps = if frame_ms > 0. { 1000. / frame_ms } else { 0. };

        let summary = format!(
            "{fps:.0} fps ({frame_ms:.1} ms/frame)\n\
             {} at zoom {:.1}\n\
             space to {}, +/- to zoom",
            self.source,
            self.memory.zoom(),
            if self.scrolling { "stop" } else { "scroll" },
        );

        Window::new("Stats")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(Align2::LEFT_TOP, [10., 10.])
            .show(ui.ctx(), |ui| {
                // Monospace, so that the window does not resize every time a digit does.
                ui.monospace(summary);
            });
    }
}

fn main() -> eframe::Result<()> {
    env_logger::init();

    eframe::run_native(
        "Walkers perf: scrolling the map",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size(WINDOW_SIZE),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(ScrollPerf::new(cc.egui_ctx.to_owned())))),
    )
}
