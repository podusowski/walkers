use std::path::{Path, PathBuf};

use log::warn;
use walkers::{Map, Stats, StyledSources, sources::Attribution};

type Build = Box<dyn Fn(&egui::Context) -> StyledSources>;

/// Basemaps to choose from, each a style drawn from its sources. Only the selected one is built, so
/// that the others do not start loading tiles nobody looks at.
pub struct Basemaps {
    egui_ctx: egui::Context,
    available: Vec<(String, Build)>,
    selected: usize,

    /// `None` until the selected basemap is needed.
    built: Option<StyledSources>,
}

impl Basemaps {
    pub fn new(egui_ctx: egui::Context) -> Self {
        Self {
            egui_ctx,
            available: Vec::new(),
            selected: 0,
            built: None,
        }
    }

    /// The first one added is selected.
    pub fn add(
        &mut self,
        name: impl Into<String>,
        build: impl Fn(&egui::Context) -> StyledSources + 'static,
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
                self.built = None;
            }
            None => warn!("There is no basemap named {name:?}."),
        }
    }

    pub fn selected_mut(&mut self) -> Option<&mut StyledSources> {
        if self.built.is_none() {
            self.built = self
                .available
                .get(self.selected)
                .map(|(_, build)| build(&self.egui_ctx));
        }

        self.built.as_mut()
    }

    /// Of the selected basemap. Empty until it is first built.
    pub fn attributions(&self) -> Vec<Attribution> {
        self.built
            .as_ref()
            .map(StyledSources::attributions)
            .unwrap_or_default()
    }

    /// Of the selected basemap.
    pub fn stats(&self) -> Stats {
        self.built
            .as_ref()
            .map(StyledSources::stats)
            .unwrap_or_default()
    }

    /// Put the selected basemap on the map.
    pub fn add_to<'a, 'b, 'c>(&'b mut self, map: Map<'a, 'b, 'c>) -> Map<'a, 'b, 'c> {
        match self.selected_mut() {
            Some(basemap) => map.with_layer(basemap, 1.0),
            None => map,
        }
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
