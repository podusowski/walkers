//! Mapterhorn's elevation shaded into relief, and traced into contour lines, over OpenFreeMap's
//! dark style, with the hillshade's paint at hand to play with.
//!
//! ```text
//! cargo run --release -p demo --bin relief --features mvt
//! ```

use egui::{Align2, Color32, Slider, Ui, Window, color_picker::Alpha};
use walkers::{
    Color, DemEncoding, Filter, Float, HttpOptions, HttpSource, Layout, Map, MapMemory, Paint,
    Position, Style, StyleLayer, StyledSources, json, lon_lat,
    sources::{Mapterhorn, OpenFreeMap},
};

/// Śnieżka, in the Karkonosze, so that there is relief to look at from the start.
fn start() -> Position {
    lon_lat(15.7400, 50.7360)
}

const STARTING_ZOOM: f64 = 12.0;

/// The hillshade's paint, and whether there are contour lines, as the window edits them.
#[derive(Clone, PartialEq)]
struct Relief {
    contours: bool,
    contour_color: Color32,
    exaggeration: f32,
    /// Where the light comes from, in degrees clockwise from the north.
    illumination_direction: f32,
    shadow: Color32,
    highlight: Color32,
    accent: Color32,
}

/// The style's defaults.
impl Default for Relief {
    fn default() -> Self {
        Self {
            contours: true,
            contour_color: Color32::from_rgb(51, 68, 51),
            exaggeration: 0.5,
            illumination_direction: 335.0,
            shadow: Color32::BLACK,
            highlight: Color32::WHITE,
            accent: Color32::BLACK,
        }
    }
}

impl Relief {
    fn style(&self) -> Style {
        let mut style = Style::openmaptiles_basemap_dark();
        let ground = ground(&style);
        style.layers.insert(
            ground,
            StyleLayer::Hillshade {
                source: Some("terrain".to_owned()),
                paint: Paint {
                    hillshade_exaggeration: Some(Float(json!(self.exaggeration))),
                    hillshade_illumination_direction: Some(Float(json!(
                        self.illumination_direction
                    ))),
                    hillshade_shadow_color: Some(color(self.shadow)),
                    hillshade_highlight_color: Some(color(self.highlight)),
                    hillshade_accent_color: Some(color(self.accent)),
                    ..Default::default()
                },
            },
        );

        // Over the relief, under roads.
        if self.contours {
            let over = ground + 1;
            style
                .layers
                .splice(over..over, contours(self.contour_color));
        }

        style
    }

    /// Whether anything changed.
    fn edit(&mut self, ui: &mut Ui) -> bool {
        let before = self.clone();

        egui::Grid::new("relief").num_columns(2).show(ui, |ui| {
            ui.label("Contour lines");
            ui.checkbox(&mut self.contours, "");
            ui.end_row();

            ui.label("Contour color");
            ui.add_enabled_ui(self.contours, |ui| {
                egui::color_picker::color_edit_button_srgba(
                    ui,
                    &mut self.contour_color,
                    Alpha::Opaque,
                );
            });
            ui.end_row();

            ui.label("Exaggeration");
            ui.add(Slider::new(&mut self.exaggeration, 0.0..=1.0));
            ui.end_row();

            ui.label("Light from");
            ui.add(Slider::new(&mut self.illumination_direction, 0.0..=359.0).suffix("°"));
            ui.end_row();

            for (name, color) in [
                ("Shadow", &mut self.shadow),
                ("Highlight", &mut self.highlight),
                ("Accent", &mut self.accent),
            ] {
                ui.label(name);
                egui::color_picker::color_edit_button_srgba(ui, color, Alpha::OnlyBlend);
                ui.end_row();
            }
        });

        if ui.button("Reset").clicked() {
            *self = Self::default();
        }

        *self != before
    }
}

/// Contour lines of the terrain in `base` color, the major ones thicker, more opaque and labelled
/// with their elevation.
fn contours(base: Color32) -> [StyleLayer; 2] {
    let level = |level: i32| Some(Filter(json!(["==", ["get", "level"], level])));
    let lines = |width: f32, opacity: f32| Paint {
        line_color: Some(color(base.gamma_multiply(opacity))),
        line_width: Some(Float(json!(width))),
        ..Default::default()
    };

    [
        StyleLayer::Contour {
            source: Some("terrain".to_owned()),
            filter: level(0),
            layout: Layout::default(),
            paint: Some(lines(0.6, 0.3)),
        },
        StyleLayer::Contour {
            source: Some("terrain".to_owned()),
            filter: level(1),
            layout: Layout {
                text_field: Some(json!(["to-string", ["get", "ele"]])),
                text_size: Some(Float(json!(12.0))),
                ..Default::default()
            },
            paint: Some(Paint {
                text_color: Some(color(base.gamma_multiply(0.8))),
                text_halo_color: Some(Color(json!("rgba(0, 0, 0, 0.6)"))),
                text_halo_width: Some(Float(json!(1.0))),
                ..lines(1.2, 0.5)
            }),
        },
    ]
}

/// As a style writes it.
fn color(color: Color32) -> Color {
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    Color(json!(format!("rgba({r}, {g}, {b}, {})", a as f32 / 255.0)))
}

/// Where layers go to be over land and water, but under everything else.
fn ground(style: &Style) -> usize {
    style
        .layers
        .iter()
        .rposition(|layer| {
            matches!(layer, StyleLayer::Fill { source_layer, .. }
                if source_layer.matches("landuse") || source_layer.matches("water"))
        })
        .map_or(0, |index| index + 1)
}

struct ReliefDemo {
    memory: MapMemory,
    map: StyledSources,
    relief: Relief,

    /// Changed in the window, but not given to the map yet, as restyling every tile at each step
    /// of a slider being dragged would only keep the styling thread busy.
    pending: bool,
}

impl ReliefDemo {
    fn new(egui_ctx: egui::Context) -> Self {
        let mut memory = MapMemory::default();
        let _ = memory.set_zoom(STARTING_ZOOM);

        let relief = Relief::default();
        let map = StyledSources::new(relief.style(), egui_ctx.clone())
            .with_tiled_source(
                "openmaptiles",
                HttpSource::new(OpenFreeMap, http_options(), egui_ctx.clone()),
            )
            .with_tiled_source(
                "terrain",
                HttpSource::new(Mapterhorn, http_options(), egui_ctx)
                    .with_elevation(DemEncoding::Terrarium),
            );

        Self {
            memory,
            map,
            relief,
            pending: false,
        }
    }
}

fn http_options() -> HttpOptions {
    HttpOptions {
        cache: Some(".cache".into()),
        ..Default::default()
    }
}

impl eframe::App for ReliefDemo {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                Map::new(None, &mut self.memory, start())
                    .with_layer(&mut self.map, 1.0)
                    .show(ui, |_, _, _, _| {});
            });

        Window::new("Relief")
            .resizable(false)
            .anchor(Align2::RIGHT_TOP, [-10., 10.])
            .show(ui.ctx(), |ui| {
                self.pending |= self.relief.edit(ui);
            });

        if self.pending && !ui.input(|input| input.pointer.any_down()) {
            self.map.set_style(self.relief.style());
            self.pending = false;
        }

        Window::new("Attribution")
            .title_bar(false)
            .resizable(false)
            .anchor(Align2::LEFT_BOTTOM, [10., -10.])
            .show(ui.ctx(), |ui| {
                for attribution in self.map.attributions() {
                    ui.hyperlink_to(attribution.text, attribution.url);
                }
            });
    }
}

fn main() -> eframe::Result<()> {
    env_logger::init();

    eframe::run_native(
        "Walkers: relief",
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            walkers::install_renderer(cc.wgpu_render_state.as_ref());
            Ok(Box::new(ReliefDemo::new(cc.egui_ctx.to_owned())))
        }),
    )
}
