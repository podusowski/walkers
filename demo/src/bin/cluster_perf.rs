use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use egui::{self, Align2, Color32, Stroke};
use rand::{Rng, SeedableRng, rngs::StdRng};

use walkers::sources;
use walkers::{HttpOptions, HttpTiles, Map, MapMemory, Position, Projector, lon_lat};
use walkers_extras::{Group, GroupedPlacesTree, LabeledSymbol, LabeledSymbolStyle, Place, Symbol};

const POI_COUNT: usize = 2_000;
const HALF_WIDTH_M: f64 = 1_200.0;
const RADIUS_PX: f32 = 72.0;

#[derive(Clone, Copy, Default)]
struct ClusterStats {
    clusters: usize,
    max_size: usize,
}

impl ClusterStats {
    fn new(clusters: usize, max_size: usize) -> Self {
        Self { clusters, max_size }
    }
}

#[derive(Default)]
struct StatsCell(Mutex<ClusterStats>);

impl StatsCell {
    fn set(&self, value: ClusterStats) {
        *self.0.lock().unwrap() = value;
    }

    fn get(&self) -> ClusterStats {
        *self.0.lock().unwrap()
    }
}

struct RollingAvg<const N: usize> {
    buf: [f64; N],
    i: usize,
    n: usize,
}

impl<const N: usize> RollingAvg<N> {
    fn new() -> Self {
        Self {
            buf: [0.0; N],
            i: 0,
            n: 0,
        }
    }

    fn reset(&mut self) {
        self.i = 0;
        self.n = 0;
        self.buf.fill(0.0);
    }

    fn push_ms(&mut self, v_ms: f64) {
        self.buf[self.i % N] = v_ms;
        self.i += 1;
        self.n = self.n.saturating_add(1).min(N);
    }

    fn mean(&self) -> f64 {
        let n = self.n.max(1);
        self.buf[..n].iter().sum::<f64>() / n as f64
    }
}

impl<const N: usize> Default for RollingAvg<N> {
    fn default() -> Self {
        Self::new()
    }
}

fn meters_to_deg_lat(m: f64) -> f64 {
    m / 111_000.0
}

fn meters_to_deg_lon(m: f64, at_lat_deg: f64) -> f64 {
    let scale = 111_000.0 * (at_lat_deg.to_radians().cos()).max(1e-6);
    m / scale
}

fn generate_poi(rng: &mut StdRng, center: Position) -> Vec<LabeledSymbol> {
    let center_lon = center.x();
    let center_lat = center.y();
    let dlat = meters_to_deg_lat(HALF_WIDTH_M);
    let dlon = meters_to_deg_lon(HALF_WIDTH_M, center_lat);

    let mut out = Vec::with_capacity(POI_COUNT);
    for i in 0..POI_COUNT {
        let lon = rng.random_range((center_lon - dlon)..(center_lon + dlon));
        let lat = rng.random_range((center_lat - dlat)..(center_lat + dlat));

        out.push(LabeledSymbol {
            position: lon_lat(lon, lat),
            label: format!("POI #{:04}", i + 1),
            symbol: Some(Symbol::Circle("•".to_string())),
            style: LabeledSymbolStyle {
                symbol_size: 5.0,
                ..LabeledSymbolStyle::default()
            },
        });
    }
    out
}

struct ClusterApp {
    memory: MapMemory,
    rng: StdRng,
    points: Vec<LabeledSymbol>,
    tiles: Option<HttpTiles>,
    avg_frame_ms: RollingAvg<120>,
    plugin: Option<Rc<GroupedPlacesTree<LabeledSymbol, DemoClusterGroup>>>,
    stats: Arc<StatsCell>,
}

impl ClusterApp {
    fn new(ctx: &egui::Context) -> Self {
        let mut app = Self {
            memory: MapMemory::default(),
            rng: StdRng::from_os_rng(),
            points: Vec::new(),
            tiles: Some(HttpTiles::with_options(
                sources::OpenStreetMap,
                HttpOptions::default(),
                ctx.clone(),
            )),
            avg_frame_ms: RollingAvg::default(),
            plugin: None,
            stats: Arc::new(StatsCell::default()),
        };
        app.regenerate_points();
        app
    }

    fn map_center() -> Position {
        lon_lat(17.03664, 51.09916)
    }

    fn regenerate_points(&mut self) {
        self.points = generate_poi(&mut self.rng, Self::map_center());
        self.avg_frame_ms.reset();
        self.stats.set(ClusterStats::default());
        self.rebuild_plugin();
    }

    fn rebuild_plugin(&mut self) {
        let plugin = GroupedPlacesTree::new(self.points.clone(), DemoClusterGroup)
            .with_screen_radius_px(RADIUS_PX)
            .viewport_only(true)
            .include_offscreen_neighbors(true)
            .with_max_group_size(None);
        self.plugin = Some(Rc::new(plugin));
    }
}

impl eframe::App for ClusterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("controls").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("R-tree clustering");
                ui.separator();
                ui.label(format!("{:.1} ms/frame", self.avg_frame_ms.mean()));
                ui.separator();
                ui.label(format!("POI: {POI_COUNT}"));
                ui.separator();
                ui.label(format!("Zoom: {:.1}", self.memory.zoom()));
                if ui.button("Zoom +").clicked() {
                    let _ = self.memory.zoom_in();
                }
                if ui.button("Zoom -").clicked() {
                    let _ = self.memory.zoom_out();
                }
                if ui.button("Regenerate").clicked() {
                    self.regenerate_points();
                }
            });

            let stats = self.stats.get();
            ui.separator();
            ui.label(format!(
                "{} clusters (max size {})",
                stats.clusters, stats.max_size
            ));
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.plugin.is_none() {
                self.rebuild_plugin();
            }

            let mut map = Map::new(None, &mut self.memory, Self::map_center());
            if let Some(tiles) = self.tiles.as_mut() {
                map = map.with_layer(tiles, 1.0);
            }

            let stats_handle = StatsHandle {
                inner: self.plugin.as_ref().expect("plugin ready").clone(),
                stats: self.stats.clone(),
            };

            let t0 = Instant::now();
            let map_response = map.with_plugin(stats_handle).show(ui, |_, _, _| {});
            let dt_ms = t0.elapsed().as_secs_f64() * 1_000.0;
            self.avg_frame_ms.push_ms(dt_ms);

            let mean = self.avg_frame_ms.mean();
            let fps = if mean > 0.0 { 1000.0 / mean } else { 0.0 };
            let mut summary =
                format!("R-tree clustering\n{mean:.1} ms/frame (~{fps:.0} fps)\nPOI: {POI_COUNT}");
            let stats = self.stats.get();
            summary.push_str(&format!(
                "\nClusters: {} (max size {})",
                stats.clusters, stats.max_size
            ));

            let painter = ui.painter_at(map_response.response.rect);
            painter.text(
                map_response.response.rect.left_top() + egui::vec2(8.0, 8.0),
                Align2::LEFT_TOP,
                summary,
                egui::TextStyle::Body.resolve(ui.style()),
                ui.style().visuals.text_color(),
            );
        });

        if ctx.input(|i| i.modifiers.alt) {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(16));
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Equals)) {
            let _ = self.memory.zoom_in();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Minus)) {
            let _ = self.memory.zoom_out();
        }
    }
}

#[derive(Clone)]
struct StatsHandle {
    inner: Rc<GroupedPlacesTree<LabeledSymbol, DemoClusterGroup>>,
    stats: Arc<StatsCell>,
}

impl walkers::Plugin for StatsHandle {
    fn run(
        self: Box<Self>,
        ui: &mut egui::Ui,
        response: &egui::Response,
        projector: &walkers::Projector,
        memory: &MapMemory,
    ) {
        let (clusters, max_size) = self.inner.draw_with_stats(ui, response, projector, memory);
        self.stats.set(ClusterStats::new(clusters, max_size));
    }
}

#[derive(Clone, Copy)]
struct DemoClusterGroup;

impl Group for DemoClusterGroup {
    fn draw<T: Place>(
        &self,
        places: &[&T],
        position: Position,
        projector: &Projector,
        ui: &mut egui::Ui,
    ) {
        let count = places.len();
        let screen = projector.project(position).to_pos2();
        let painter = ui.painter();

        let (fill, stroke_color) = cluster_palette(count);
        let radius = 18.0 + (count as f32).sqrt() * 6.0;

        painter.circle_filled(screen, radius, fill);
        painter.circle_stroke(screen, radius, Stroke::new(3.0, stroke_color));

        painter.text(
            screen,
            Align2::CENTER_CENTER,
            count.to_string(),
            egui::TextStyle::Heading.resolve(ui.style()),
            Color32::WHITE,
        );
    }
}

fn cluster_palette(count: usize) -> (Color32, Color32) {
    match count {
        0..=4 => (
            Color32::from_rgb(0x3b, 0xd9, 0x85),
            Color32::from_rgb(0x17, 0xa6, 0x5c),
        ),
        5..=15 => (
            Color32::from_rgb(0xff, 0xbd, 0x6b),
            Color32::from_rgb(0xe3, 0x78, 0x15),
        ),
        16..=48 => (
            Color32::from_rgb(0xff, 0x87, 0x7b),
            Color32::from_rgb(0xd7, 0x36, 0x35),
        ),
        49..=120 => (
            Color32::from_rgb(0xad, 0x8c, 0xff),
            Color32::from_rgb(0x6f, 0x52, 0xd4),
        ),
        _ => (
            Color32::from_rgb(0x4e, 0x63, 0xf0),
            Color32::from_rgb(0x24, 0x34, 0xb3),
        ),
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Walkers perf: R-tree clustering",
        options,
        Box::new(|cc| Ok(Box::new(ClusterApp::new(&cc.egui_ctx)))),
    )
}
