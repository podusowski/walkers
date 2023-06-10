use std::{sync::Arc, thread::JoinHandle};

use egui::{Align2, FontId, RichText, Window};
use walkers::{MapMemory, Zoom};

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    eframe::run_native(
        "OpenStreetMap",
        Default::default(),
        Box::new(|_cc| Box::new(Osm::new())),
    )
}

struct TokioRuntimeThread {
    join_handle: Option<JoinHandle<()>>,
    quit_tx: tokio::sync::mpsc::UnboundedSender<()>,
    runtime: Arc<tokio::runtime::Runtime>,
}

impl TokioRuntimeThread {
    pub fn new() -> Self {
        let (quit_tx, mut quit_rx) = tokio::sync::mpsc::unbounded_channel();
        let (rt_tx, mut rt_rx) = tokio::sync::mpsc::unbounded_channel();

        let join_handle = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            let rt = Arc::new(runtime);
            rt_tx.send(rt.clone()).unwrap();
            rt.block_on(quit_rx.recv());
        });

        Self {
            join_handle: Some(join_handle),
            quit_tx,
            runtime: rt_rx.blocking_recv().unwrap(),
        }
    }
}

impl Drop for TokioRuntimeThread {
    fn drop(&mut self) {
        self.quit_tx.send(()).unwrap();

        if let Some(join_handle) = self.join_handle.take() {
            log::debug!("Waiting for the Tokio thread to exit.");
            // Not much to do if it's an error.
            _ = join_handle.join();
        }
    }
}

struct Osm {
    tiles: walkers::Tiles,
    map_memory: MapMemory,

    #[allow(dead_code)] // Significant Drop
    tokio_thread: TokioRuntimeThread,
}

impl Osm {
    fn new() -> Self {
        let tokio_thread = TokioRuntimeThread::new();
        let mut map_memory = MapMemory::default();
        map_memory.osm = true;
        Self {
            tiles: walkers::Tiles::new(tokio_thread.runtime.clone()),
            map_memory,
            tokio_thread,
        }
    }
}

impl eframe::App for Osm {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("following map uses data from");
                ui.hyperlink("https://www.openstreetmap.org");
                ui.label(", please consider donating at");
                ui.hyperlink("https://donate.openstreetmap.org/");
            });

            ui.add(walkers::Map::new(
                &mut self.tiles,
                &mut self.map_memory,
                walkers::Position::new(17.03664, 51.09916),
            ));

            Window::new("Map")
                .collapsible(false)
                .resizable(false)
                .title_bar(false)
                .anchor(Align2::LEFT_BOTTOM, [10., -10.])
                .show(ui.ctx(), |ui| {
                    ui.label(format!("current zoom: {}", *self.map_memory.zoom));
                    ui.horizontal(|ui| {
                        if ui
                            .button(RichText::new("➕").font(FontId::proportional(20.)))
                            .clicked()
                        {
                            let _ = self.map_memory.zoom.try_zoom_in();
                        }

                        if ui
                            .button(RichText::new("➖").font(FontId::proportional(20.)))
                            .clicked()
                        {
                            if let Ok(zoom) = Zoom::try_from(*self.map_memory.zoom - 1) {
                                self.map_memory.zoom = zoom;
                            }
                        }
                    });
                });
        });
    }
}
