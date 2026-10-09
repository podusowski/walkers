use std::path::{Path, PathBuf};

use egui::{Align2, Color32, ComboBox, Image, Key, PointerButton, RichText, Ui, Window};
use walkers::{
    HttpOptions, HttpTiles, Map, MapMemory, MercatorProjection, PmTiles, Position, Projection,
    Style, Tiles, lon_lat, sources,
};
use walkers_extras::{
    Basemaps, GroupedPlaces, LabeledSymbol, LabeledSymbolGroup, LabeledSymbolGroupStyle,
    LabeledSymbolStyle, Symbol, pmtiles_files,
};

use crate::journal::{self, Journal, Place};

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

/// Maps the places can be drawn on top of. Protomaps only shows up when there is a file for it.
fn basemaps(egui_ctx: egui::Context) -> Basemaps {
    let mut basemaps = Basemaps::new(egui_ctx);

    basemaps.add("OpenStreetMap", |ctx| {
        vec![(http(sources::OpenStreetMap, None, ctx), 1.0)]
    });
    basemaps.add("OpenFreeMap (light)", |ctx| {
        let style = Style::openmaptiles_basemap_light();
        vec![(http(sources::OpenFreeMap, Some(style), ctx), 1.0)]
    });
    basemaps.add("OpenFreeMap (dark)", |ctx| {
        let style = Style::openmaptiles_basemap_dark();
        vec![(http(sources::OpenFreeMap, Some(style), ctx), 1.0)]
    });
    basemaps.add("Geoportal", |ctx| {
        vec![(http(sources::Geoportal, None, ctx), 1.0)]
    });

    // See `just protomaps-dolnoslaskie` in `walkers` for one way of getting a file.
    for path in pmtiles_files(Path::new(".")) {
        let name = path.file_stem().unwrap_or_default().to_string_lossy();

        let light = path.to_owned();
        basemaps.add(format!("{name} (light)"), move |ctx| {
            let style = Style::protomaps_basemap_light();
            vec![(pmtiles(&light, style, ctx), 1.0)]
        });

        let dark = path.to_owned();
        basemaps.add(format!("{name} (dark)"), move |ctx| {
            let style = Style::protomaps_basemap_dark();
            vec![(pmtiles(&dark, style, ctx), 1.0)]
        });
    }

    basemaps
}

fn http(
    source: impl sources::TileSource<Projection = MercatorProjection> + Send + Sync + 'static,
    style: Option<Style>,
    egui_ctx: &egui::Context,
) -> Box<dyn Tiles<Projection = MercatorProjection>> {
    let egui_ctx = egui_ctx.to_owned();
    Box::new(match style {
        Some(style) => HttpTiles::with_options_and_style(source, http_options(), style, egui_ctx),
        None => HttpTiles::with_options(source, http_options(), egui_ctx),
    })
}

fn pmtiles(
    path: &Path,
    style: Style,
    egui_ctx: &egui::Context,
) -> Box<dyn Tiles<Projection = MercatorProjection>> {
    Box::new(PmTiles::with_style(
        path,
        MercatorProjection,
        style,
        egui_ctx.to_owned(),
    ))
}

/// A place clicked on the map, waiting to be named and added to the journal.
struct NewPlace {
    position: Position,
    name: String,
    asked_for_focus: bool,
}

enum Outcome {
    StillTyping,
    Add,
    Cancel,
}

pub struct Wanderers {
    /// Only the selected one is kept around. Switching back and forth costs a rebuild, but the
    /// tiles themselves are cached on disk, so it is not a re-download.
    basemaps: Basemaps,
    map_memory: MapMemory,
    journal: Result<Journal, journal::Error>,
    new_place: Option<NewPlace>,

    /// Something which went wrong after the journal was read, such as a failed save.
    problem: Option<String>,
}

impl Wanderers {
    pub fn new(egui_ctx: egui::Context, journal: PathBuf) -> Self {
        Self {
            basemaps: basemaps(egui_ctx),
            map_memory: MapMemory::default(),
            journal: Journal::load(journal),
            new_place: None,
            problem: None,
        }
    }

    /// Every addition is written out right away, so that there is no unsaved journal to lose.
    fn add(&mut self, new_place: NewPlace) {
        let Ok(journal) = &mut self.journal else {
            return;
        };

        journal
            .places
            .push(Place::new(new_place.position, new_place.name));

        self.problem = journal.save().err().map(|error| error.to_string());
    }
}

impl eframe::App for Wanderers {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let pending = self.new_place.as_ref().map(|new_place| new_place.position);

        let map = Map::new(&mut self.map_memory, home()).zoom_with_ctrl(false);
        let mut map = self.basemaps.add_to(map);

        if let Ok(journal) = &self.journal {
            map = map.with_plugin(places(&journal.places));
        }

        let clicked_at = map
            .show(ui, |ui, response, projector, _| {
                // Drawn outside of the grouped places, so that it does not disappear into a
                // cluster while it is still being named.
                if let Some(position) = pending {
                    ui.painter().circle_filled(
                        projector.project(position).to_pos2(),
                        7.,
                        Color32::from_rgb(0xE0, 0x6C, 0x00),
                    );
                }

                // `changed` tells a click apart from the end of a drag across the map.
                if response.changed() || !response.clicked_by(PointerButton::Primary) {
                    return None;
                }

                response
                    .interact_pointer_pos()
                    .map(|clicked_at| projector.unproject(clicked_at.to_vec2()))
            })
            .inner;

        // Clicking again moves the place being named, rather than starting over.
        if let Some(position) = clicked_at {
            match &mut self.new_place {
                Some(new_place) => new_place.position = position,
                None => {
                    self.new_place = Some(NewPlace {
                        position,
                        name: String::new(),
                        asked_for_focus: false,
                    })
                }
            }
        }

        if let Some(mut new_place) = self.new_place.take() {
            match ask_for_name(ui, &mut new_place) {
                Outcome::StillTyping => self.new_place = Some(new_place),
                Outcome::Add => self.add(new_place),
                Outcome::Cancel => {}
            }
        }

        basemap(ui, &mut self.basemaps);
        zoom(ui, &mut self.map_memory);
        go_home(ui, &mut self.map_memory);
        acknowledge(ui, self.basemaps.attributions());

        match (&self.journal, &self.problem) {
            (Err(error), _) => complain(ui, &error.to_string()),
            (_, Some(problem)) => complain(ui, problem),
            _ => {}
        }
    }
}

/// Asks for the name of a place just clicked on the map.
fn ask_for_name(ui: &Ui, new_place: &mut NewPlace) -> Outcome {
    let mut outcome = Outcome::StillTyping;

    Window::new("New place")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::CENTER_BOTTOM, [0., -10.])
        .show(ui.ctx(), |ui| {
            ui.label(format!(
                "{:.04} {:.04}",
                new_place.position.x(),
                new_place.position.y()
            ));

            let name = ui.text_edit_singleline(&mut new_place.name);

            if !new_place.asked_for_focus {
                new_place.asked_for_focus = true;
                name.request_focus();
            }

            if name.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
                outcome = Outcome::Add;
            }

            ui.horizontal(|ui| {
                if ui.button("Add").clicked() {
                    outcome = Outcome::Add;
                }

                if ui.button("Cancel").clicked() || ui.input(|input| input.key_pressed(Key::Escape))
                {
                    outcome = Outcome::Cancel;
                }
            });
        });

    outcome
}

/// Places are grouped, so that a journal which got dense in one city is still readable when
/// the whole country is on the screen.
fn places<P: Projection>(places: &[journal::Place]) -> impl walkers::Plugin<P> {
    GroupedPlaces::new(
        places
            .iter()
            .map(|place| LabeledSymbol {
                position: place.position,
                label: place.name.to_owned(),
                symbol: Some(Symbol::Circle("📍".to_owned())),
                style: LabeledSymbolStyle {
                    symbol_size: 20.,
                    ..Default::default()
                },
            })
            .collect(),
        LabeledSymbolGroup {
            style: LabeledSymbolGroupStyle::default(),
        },
    )
}

/// Lets one swap the map the places are drawn on.
fn basemap(ui: &Ui, basemaps: &mut Basemaps) {
    let mut chosen = basemaps.selected().to_owned();

    Window::new("Basemap")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::RIGHT_TOP, [-10., 10.])
        .show(ui.ctx(), |ui| {
            ComboBox::from_id_salt("basemap")
                .selected_text(&chosen)
                .show_ui(ui, |ui| {
                    for name in basemaps.names() {
                        ui.selectable_value(&mut chosen, name.to_owned(), name);
                    }
                });
        });

    basemaps.select(&chosen);
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
fn go_home(ui: &Ui, memory: &mut MapMemory) {
    if memory.detached().is_some() {
        Window::new("Go home")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(Align2::RIGHT_BOTTOM, [-10., -10.])
            .show(ui.ctx(), |ui| {
                if ui.button(RichText::new("go back home").heading()).clicked() {
                    memory.follow_my_position();
                }
            });
    }
}

/// A journal we failed to read is complained about, and not quietly treated as an empty one,
/// which would risk overwriting it with nothing.
fn complain(ui: &Ui, problem: &str) {
    Window::new("Journal")
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0., 0.])
        .show(ui.ctx(), |ui| {
            ui.label(problem);
        });
}

/// OpenStreetMap requires the map to be credited, wherever it is shown.
fn acknowledge(ui: &Ui, attributions: Vec<sources::Attribution>) {
    Window::new("Acknowledge")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::LEFT_TOP, [10., 10.])
        .show(ui.ctx(), |ui| {
            for attribution in attributions {
                ui.horizontal(|ui| {
                    if let Some(logo) = attribution.logo_light {
                        ui.add(Image::new(logo).max_height(30.).max_width(80.));
                    }
                    ui.hyperlink_to(attribution.text, attribution.url);
                });
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vector maps come in both shades, and each is its own entry in the picker.
    #[test]
    fn every_shade_of_a_vector_basemap_is_offered_separately() {
        let basemaps = basemaps(egui::Context::default());
        let names: Vec<_> = basemaps.names().collect();

        assert!(names.contains(&"OpenFreeMap (light)"));
        assert!(names.contains(&"OpenFreeMap (dark)"));

        // Raster ones have nothing to shade, so they stay single.
        assert_eq!(
            names
                .iter()
                .filter(|name| **name == "OpenStreetMap")
                .count(),
            1
        );
    }

    /// Two entries which look the same in the picker would be indistinguishable to click.
    #[test]
    fn basemap_names_are_unique() {
        let basemaps = basemaps(egui::Context::default());
        let names: Vec<_> = basemaps.names().collect();
        let unique: std::collections::HashSet<_> = names.iter().collect();

        assert_eq!(names.len(), unique.len(), "duplicate names in {names:?}");
    }
}
