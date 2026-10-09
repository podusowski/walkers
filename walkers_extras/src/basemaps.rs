use std::path::{Path, PathBuf};

use log::warn;
use walkers::{Map, MercatorProjection, Projection, Stats, Tiles, sources::Attribution};

type Layers<P> = Vec<(Box<dyn Tiles<Projection = P>>, f32)>;
type Build<P> = Box<dyn Fn(&egui::Context) -> Layers<P>>;

/// Basemaps to choose from, each a stack of tile layers with their transparency. Only the
/// selected one is built, so that the others do not start loading tiles nobody looks at.
pub struct Basemaps<P: Projection = MercatorProjection> {
    egui_ctx: egui::Context,
    available: Vec<(String, Build<P>)>,
    selected: usize,

    /// `None` until the selected basemap is needed.
    layers: Option<Layers<P>>,
}

impl<P: Projection> Basemaps<P> {
    pub fn new(egui_ctx: egui::Context) -> Self {
        Self {
            egui_ctx,
            available: Vec::new(),
            selected: 0,
            layers: None,
        }
    }

    /// The first one added is selected.
    pub fn add(
        &mut self,
        name: impl Into<String>,
        build: impl Fn(&egui::Context) -> Layers<P> + 'static,
    ) {
        self.available.push((name.into(), Box::new(build)));
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.available.iter().map(|(name, _)| name.as_str())
    }

    /// Empty if none was added.
    pub fn selected(&self) -> &str {
        self.available
            .get(self.selected)
            .map_or("", |(name, _)| name)
    }

    pub fn select(&mut self, name: &str) {
        match self.available.iter().position(|(n, _)| n == name) {
            Some(index) if index == self.selected => {}
            Some(index) => {
                self.selected = index;
                self.layers = None;
            }
            None => warn!("There is no basemap named {name:?}."),
        }
    }

    /// Of the selected basemap. Empty until it is first added to a map.
    pub fn attributions(&self) -> Vec<Attribution> {
        self.layers
            .iter()
            .flatten()
            .map(|(tiles, _)| tiles.attribution())
            .collect()
    }

    /// Summed over the layers of the selected basemap.
    pub fn stats(&self) -> Stats {
        Stats {
            in_progress: self
                .layers
                .iter()
                .flatten()
                .map(|(tiles, _)| tiles.stats().in_progress)
                .sum(),
        }
    }

    /// Put the layers of the selected basemap on the map.
    pub fn add_to<'a, 'b, 'c>(&'b mut self, mut map: Map<'a, 'b, 'c, P>) -> Map<'a, 'b, 'c, P> {
        let layers = self.layers.get_or_insert_with(|| {
            self.available
                .get(self.selected)
                .map(|(_, build)| build(&self.egui_ctx))
                .unwrap_or_default()
        });

        for (tiles, transparency) in layers {
            map = map.with_layer(tiles.as_mut(), *transparency);
        }

        map
    }
}

/// `.pmtiles` files in `dir`, sorted by name.
pub fn pmtiles_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut files: Vec<_> = entries
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            (path.extension()? == "pmtiles").then_some(path)
        })
        .collect();
    files.sort();
    files
}
