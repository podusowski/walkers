use std::path::PathBuf;

use egui::{Align2, Color32, ComboBox, Image, Key, PointerButton, RichText, Ui, Window};
use walkers::{HttpOptions, HttpTiles, Map, MapMemory, PmTiles, Position, Tiles, lon_lat, sources};
use walkers_extras::{
    GroupedPlaces, LabeledSymbol, LabeledSymbolGroup, LabeledSymbolGroupStyle, LabeledSymbolStyle,
    Symbol,
};

use crate::journal::{self, Journal, Place};
use crate::map_style::{self, Shade};

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

/// Map the places are drawn on top of.
#[derive(Clone, PartialEq)]
enum Basemap {
    OpenStreetMap,

    /// Vector tiles, so the same map comes in either shade.
    OpenFreeMap(Shade),

    /// Poland only, but it has aerial imagery, which the other two do not.
    Geoportal,

    /// A `.pmtiles` file sitting next to the app, so the map works without a network.
    Protomaps(PathBuf, Shade),
}

impl Basemap {
    /// In the order they are offered. Protomaps only shows up when there is a file for it.
    fn all() -> Vec<Basemap> {
        let mut all = vec![Basemap::OpenStreetMap];
        all.extend(Shade::ALL.map(Basemap::OpenFreeMap));
        all.push(Basemap::Geoportal);

        for path in pmtiles_files() {
            all.extend(Shade::ALL.map(|shade| Basemap::Protomaps(path.to_owned(), shade)));
        }

        all
    }

    fn name(&self) -> String {
        match self {
            Basemap::OpenStreetMap => "OpenStreetMap".to_owned(),
            Basemap::OpenFreeMap(shade) => format!("OpenFreeMap ({})", shade.name()),
            Basemap::Geoportal => "Geoportal".to_owned(),
            Basemap::Protomaps(path, shade) => format!(
                "{} ({})",
                path.file_stem().unwrap_or_default().to_string_lossy(),
                shade.name()
            ),
        }
    }

    fn tiles(&self, egui_ctx: egui::Context) -> Basetiles {
        match self {
            Basemap::OpenStreetMap => Basetiles::Http(Box::new(HttpTiles::with_options(
                sources::OpenStreetMap,
                http_options(),
                egui_ctx,
            ))),
            Basemap::OpenFreeMap(shade) => {
                Basetiles::Http(Box::new(HttpTiles::with_options_and_style(
                    sources::OpenFreeMap,
                    http_options(),
                    map_style::style(*shade, map_style::OPENMAPTILES),
                    egui_ctx,
                )))
            }
            Basemap::Geoportal => Basetiles::Http(Box::new(HttpTiles::with_options(
                sources::Geoportal,
                http_options(),
                egui_ctx,
            ))),
            Basemap::Protomaps(path, shade) => Basetiles::PmTiles(Box::new(PmTiles::with_style(
                path,
                map_style::style(*shade, map_style::PROTOMAPS),
                egui_ctx,
            ))),
        }
    }
}

/// `.pmtiles` files in the current directory. See `just protomaps-dolnoslaskie` in `walkers`
/// for one way of getting one.
fn pmtiles_files() -> Vec<PathBuf> {
    let Ok(dir) = std::fs::read_dir(".") else {
        return Vec::new();
    };

    let mut files: Vec<_> = dir
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            (path.extension()?.to_str()? == "pmtiles").then_some(path)
        })
        .collect();
    files.sort();
    files
}

/// Tiles either come over HTTP or out of a local file.
enum Basetiles {
    Http(Box<HttpTiles>),
    PmTiles(Box<PmTiles>),
}

impl Basetiles {
    fn as_mut(&mut self) -> &mut dyn Tiles {
        match self {
            Basetiles::Http(tiles) => tiles.as_mut(),
            Basetiles::PmTiles(tiles) => tiles.as_mut(),
        }
    }

    fn attribution(&self) -> sources::Attribution {
        match self {
            Basetiles::Http(tiles) => tiles.attribution(),
            Basetiles::PmTiles(tiles) => tiles.attribution(),
        }
    }
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
    /// Kept so that switching the basemap can build the tiles for it.
    egui_ctx: egui::Context,

    basemap: Basemap,
    tiles: Basetiles,
    map_memory: MapMemory,
    journal: Result<Journal, journal::Error>,
    new_place: Option<NewPlace>,

    /// Something which went wrong after the journal was read, such as a failed save.
    problem: Option<String>,
}

impl Wanderers {
    pub fn new(egui_ctx: egui::Context, journal: PathBuf) -> Self {
        let basemap = Basemap::OpenStreetMap;

        Self {
            tiles: basemap.tiles(egui_ctx.to_owned()),
            egui_ctx,
            basemap,
            map_memory: MapMemory::default(),
            journal: Journal::load(journal),
            new_place: None,
            problem: None,
        }
    }

    /// Only one basemap is kept around at a time. Switching back and forth costs a rebuild,
    /// but the tiles themselves are cached on disk, so it is not a re-download.
    fn switch_basemap_to(&mut self, basemap: Basemap) {
        self.tiles = basemap.tiles(self.egui_ctx.to_owned());
        self.basemap = basemap;
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
        let attribution = self.tiles.attribution();
        let pending = self.new_place.as_ref().map(|new_place| new_place.position);

        let mut map =
            Map::new(Some(self.tiles.as_mut()), &mut self.map_memory, home()).zoom_with_ctrl(false);

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

        if let Some(chosen) = basemap(ui, &self.basemap) {
            self.switch_basemap_to(chosen);
        }

        zoom(ui, &mut self.map_memory);
        go_home(ui, &mut self.map_memory);
        acknowledge(ui, attribution);

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
fn places(places: &[journal::Place]) -> impl walkers::Plugin {
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

/// Lets one swap the map the places are drawn on. Returns the pick, if it changed.
fn basemap(ui: &Ui, current: &Basemap) -> Option<Basemap> {
    let mut chosen = current.to_owned();

    Window::new("Basemap")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::RIGHT_TOP, [-10., 10.])
        .show(ui.ctx(), |ui| {
            ComboBox::from_id_salt("basemap")
                .selected_text(current.name())
                .show_ui(ui, |ui| {
                    for basemap in Basemap::all() {
                        let name = basemap.name();
                        ui.selectable_value(&mut chosen, basemap, name);
                    }
                });
        });

    (&chosen != current).then_some(chosen)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Vector maps come in both shades, and each is its own entry in the picker.
    #[test]
    fn every_shade_of_a_vector_basemap_is_offered_separately() {
        let names: Vec<_> = Basemap::all().iter().map(Basemap::name).collect();

        assert!(names.contains(&"OpenFreeMap (light)".to_owned()));
        assert!(names.contains(&"OpenFreeMap (dark)".to_owned()));

        // Raster ones have nothing to shade, so they stay single.
        assert_eq!(
            names.iter().filter(|name| *name == "OpenStreetMap").count(),
            1
        );
    }

    /// Two entries which look the same in the picker would be indistinguishable to click.
    #[test]
    fn basemap_names_are_unique() {
        let names: Vec<_> = Basemap::all().iter().map(Basemap::name).collect();
        let unique: std::collections::HashSet<_> = names.iter().collect();

        assert_eq!(names.len(), unique.len(), "duplicate names in {names:?}");
    }
}
