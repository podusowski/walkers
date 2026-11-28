use eframe::{App, CreationContext, Frame};
use egui::Ui;
use walkers::{
    Color, Feature, FeatureLayer, Float, HttpOptions, HttpSource, IconFill, Layout, Map, MapMemory,
    MemorySource, Paint, Palette, Position, StyleLayer, StyledSources, Waymark,
    geo_types::{LineString, Polygon},
    json, lon_lat,
    sources::OpenFreeMap,
};

pub struct FeaturesMap {
    map: StyledSources,
    map_memory: MapMemory,

    /// Kept for moving the drone, while `map` draws from a clone of it.
    drone: MemorySource,

    /// When the drone last reported where it is, in seconds since the app started.
    reported: f64,
}

// ANCHOR: features
/// A walk through Wrocław's old town, and the island it ends on.
fn walk() -> Vec<FeatureLayer> {
    vec![
        FeatureLayer::new(
            "areas",
            vec![Feature::new(Polygon::new(
                LineString::from(vec![
                    (17.0420, 51.1150),
                    (17.0500, 51.1160),
                    (17.0520, 51.1130),
                    (17.0440, 51.1120),
                    (17.0420, 51.1150),
                ]),
                vec![],
            ))],
        ),
        FeatureLayer::new(
            "routes",
            vec![Feature::new(LineString::from(vec![
                (17.0320, 51.1100),
                (17.0345, 51.1130),
                (17.0430, 51.1135),
                (17.0466, 51.1142),
            ]))],
        ),
        FeatureLayer::new(
            "places",
            vec![
                Feature::new(lon_lat(17.0320, 51.1100)).with_property("name", "Start"),
                Feature::new(lon_lat(17.0466, 51.1142)).with_property("name", "Finish"),
            ],
        ),
    ]
}

/// The drone, circling over the walk.
fn drone_at(time: f64) -> Vec<FeatureLayer> {
    let angle = time * 0.05;
    let position = lon_lat(17.04 + 0.006 * angle.cos(), 51.113 + 0.004 * angle.sin());
    vec![FeatureLayer::new(
        "drones",
        vec![Feature::new(position).with_property("name", "Drone")],
    )]
}
// ANCHOR_END: features

// ANCHOR: style
/// Layers drawing the walk, from a source called `walk`, under the basemap's labels.
fn walk_layers() -> Vec<StyleLayer> {
    vec![
        StyleLayer::Fill {
            source: Some("walk".into()),
            source_layer: "areas".into(),
            filter: None,
            paint: Paint {
                fill_color: Some(Color::from("#2a9d8f")),
                fill_opacity: Some(Float(json!(0.4))),
                ..Default::default()
            },
        },
        StyleLayer::Line {
            source: Some("walk".into()),
            source_layer: "routes".into(),
            filter: None,
            paint: Paint {
                line_color: Some(Color::from("#e9c46a")),
                line_width: Some(Float(json!(4.0))),
                ..Default::default()
            },
        },
    ]
}

/// Labels of the walk and the drone, over the basemap's, so that they win where they overlap.
fn label_layers() -> Vec<StyleLayer> {
    let label = |source: &str, source_layer: &str, icon: Option<&str>| StyleLayer::Symbol {
        source: Some(source.into()),
        source_layer: source_layer.into(),
        minzoom: None,
        filter: None,
        layout: Layout {
            icon_image: icon.map(|icon| json!(icon)),
            text_field: Some(json!(["get", "name"])),
            text_anchor: Some(json!("top")),
            text_offset: Some(json!([0.0, 0.8])),
            ..Default::default()
        },
        paint: Some(Paint {
            text_color: Some(Color::from("#ffffff")),
            text_halo_color: Some(Color::from("#000000")),
            text_halo_width: Some(Float(json!(1.5))),
            ..Default::default()
        }),
    };
    vec![
        label("walk", "places", None),
        label("drone", "drones", Some("drone")),
    ]
}
// ANCHOR_END: style

// ANCHOR: new
impl FeaturesMap {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        let ctx = &cc.egui_ctx;
        let mut style = Waymark::new(Palette::dark()).build();

        let basemap_labels = style
            .layers
            .iter()
            .position(|layer| matches!(layer, StyleLayer::Symbol { .. }))
            .unwrap_or(style.layers.len());
        style
            .layers
            .splice(basemap_labels..basemap_labels, walk_layers());
        style.layers.extend(label_layers());
        style.icons.insert(
            "drone".into(),
            vec![IconFill {
                fill: vec![[0.0, -6.0], [6.0, 0.0], [0.0, 6.0], [-6.0, 0.0]],
                fill_color: Color::from("#e76f51"),
            }],
        );

        let tiles = HttpSource::new(OpenFreeMap, HttpOptions::default(), ctx.clone());
        let drone = MemorySource::new(drone_at(0.0));
        let map = StyledSources::new(style, ctx.clone())
            .with_tiled_source("openmaptiles", tiles)
            .with_source("walk", MemorySource::new(walk()))
            .with_source("drone", drone.clone());

        let mut map_memory = MapMemory::default();
        let _ = map_memory.set_zoom(15.0);

        Self {
            map,
            map_memory,
            drone,
            reported: 0.0,
        }
    }
}
// ANCHOR_END: new

// ANCHOR: ui
impl App for FeaturesMap {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut Frame) {
        // Once a second, as a real drone would report where it is.
        let second = ui.input(|input| input.time).floor();
        if second > self.reported {
            self.reported = second;
            self.drone.set(drone_at(second));
        }
        ui.ctx().request_repaint_after_secs(1.0);

        let attributions = self.map.attributions();
        let old_town: Position = lon_lat(17.04, 51.1125);

        Map::new(None, &mut self.map_memory, old_town)
            .with_layer(&mut self.map, 1.0)
            .show(ui, |ui, _, _, _| {
                for attribution in attributions {
                    ui.hyperlink_to(attribution.text, attribution.url);
                }
            });
    }
}
// ANCHOR_END: ui
