use super::{Attribution, TileSource};
use crate::TileId;

/// Orthophotomap layer from Poland's Geoportal.
/// <https://www.geoportal.gov.pl/uslugi/usluga-przegladania-wms>
pub struct Geoportal;

impl TileSource for Geoportal {
    fn tile_url(&self, tile_id: TileId) -> String {
        format!(
            "https://mapy.geoportal.gov.pl/wss/service/PZGIK/ORTO/WMTS/StandardResolution?\
            &SERVICE=WMTS\
            &REQUEST=GetTile\
            &VERSION=1.0.0\
            &LAYER=ORTOFOTOMAPA\
            &TILEMATRIXSET=EPSG:3857\
            &TILEMATRIX=EPSG:3857:{}\
            &TILEROW={}\
            &TILECOL={}",
            tile_id.zoom, tile_id.y, tile_id.x
        )
    }

    fn attribution(&self) -> Attribution {
        Attribution {
            text: "Główny Urząd Geodezji i Kartografii",
            url: "https://www.geoportal.gov.pl/",
            logo_light: None,
            logo_dark: None,
        }
    }
}
