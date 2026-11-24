use eframe::{App, CreationContext, Frame};
use egui::Ui;
use walkers::{
    HttpOptions, HttpSource, Map, MapMemory, Palette, StyledSources, Waymark, lon_lat,
    sources::OpenFreeMap,
};

pub struct WaymarkMap {
    map: StyledSources,
    map_memory: MapMemory,
}

// ANCHOR: new
impl WaymarkMap {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        let ctx = &cc.egui_ctx;
        let style = Waymark::new(Palette::dark()).build();
        let tiles = HttpSource::new(OpenFreeMap, HttpOptions::default(), ctx.clone());

        Self {
            map: StyledSources::new(style, ctx.clone()).with_tiled_source("openmaptiles", tiles),
            map_memory: MapMemory::default(),
        }
    }
}
// ANCHOR_END: new

// ANCHOR: ui
impl App for WaymarkMap {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut Frame) {
        let attributions = self.map.attributions();
        let wroclaw = lon_lat(17.03664, 51.09916);

        Map::new(None, &mut self.map_memory, wroclaw)
            .with_layer(&mut self.map, 1.0)
            .show(ui, |ui, _, _, _| {
                for attribution in attributions {
                    ui.hyperlink_to(attribution.text, attribution.url);
                }
            });
    }
}
// ANCHOR_END: ui
