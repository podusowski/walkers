use eframe::{App, CreationContext, Frame};
use egui::Ui;
use walkers::{HttpTiles, Map, MapMemory, Tiles, lon_lat, sources::OpenStreetMap};

pub struct RasterMap {
    tiles: HttpTiles,
    map_memory: MapMemory,
}

impl RasterMap {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        Self {
            tiles: HttpTiles::new(OpenStreetMap, cc.egui_ctx.clone()),
            map_memory: MapMemory::default(),
        }
    }
}

impl App for RasterMap {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut Frame) {
        let attribution = self.tiles.attribution();
        let wroclaw = lon_lat(17.03664, 51.09916);

        Map::new(Some(&mut self.tiles), &mut self.map_memory, wroclaw).show(ui, |ui, _, _, _| {
            ui.hyperlink_to(attribution.text, attribution.url);
        });
    }
}
