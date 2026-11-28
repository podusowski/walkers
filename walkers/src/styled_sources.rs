use std::{ops::Range, sync::Arc};

use egui::Context;

#[cfg(feature = "vector")]
use crate::untiled::Untiled;
use crate::{
    Canvas, Layer, Projector, Stats, Style, TilePiece, TiledSource,
    sources::Attribution,
    tiles::{TileId, draw_tiles_with, styling::Styling},
};

/// A [`Style`] drawn from named sources. It must persist between frames.
///
/// ```no_run
/// # use walkers::{HttpOptions, HttpSource, StyledSources, Style, sources::OpenStreetMap};
/// # let ctx = egui::Context::default();
/// let osm = HttpSource::new(OpenStreetMap, HttpOptions::default(), ctx.clone());
/// let map = StyledSources::new(Style::default(), ctx).with_tiled_source("osm", osm);
/// ```
pub struct StyledSources {
    style: Arc<Style>,
    sources: Vec<NamedSource>,

    /// Worked out when first needed, so that adding sources one by one does not warn about those
    /// yet to be added.
    consecutive_layers: Option<Vec<ConsecutiveLayers>>,

    styling: Styling,
    egui_ctx: Context,
}

type NamedSource = (String, Kind);

/// Each kind of source is drawn its own way.
enum Kind {
    Tiled(Box<dyn TiledSource>),
    #[cfg(feature = "vector")]
    Untiled(Untiled),
}

/// Consecutive layers of the style drawing from the same source, as the index of the source and
/// the layers. They are styled and drawn together.
type ConsecutiveLayers = (usize, Range<usize>);

impl StyledSources {
    pub fn new(style: Style, egui_ctx: Context) -> Self {
        let style = Arc::new(style);
        Self {
            styling: Styling::new(style.clone(), egui_ctx.clone()),
            style,
            sources: Vec::new(),
            consecutive_layers: None,
            egui_ctx,
        }
    }

    /// Add a source, which the style's layers can name to draw from it.
    pub fn with_tiled_source(
        mut self,
        name: impl Into<String>,
        source: impl TiledSource + 'static,
    ) -> Self {
        self.sources
            .push((name.into(), Kind::Tiled(Box::new(source))));
        self.consecutive_layers = None;
        self
    }

    /// Add a source of untiled data, which the style's layers can name to draw from it.
    #[cfg(feature = "vector")]
    pub fn with_source(
        mut self,
        name: impl Into<String>,
        source: impl crate::Source + 'static,
    ) -> Self {
        let source = Untiled::new(Box::new(source));
        self.sources.push((name.into(), Kind::Untiled(source)));
        self.consecutive_layers = None;
        self
    }

    /// Draw with another style from now on. The sources keep what they have fetched, so that
    /// nothing is fetched again.
    pub fn set_style(&mut self, style: Style) {
        self.style = Arc::new(style);
        self.styling = Styling::new(self.style.clone(), self.egui_ctx.clone());
        self.consecutive_layers = None;
    }

    /// Attributions of every source which has one, in the order they were added.
    pub fn attributions(&self) -> Vec<Attribution> {
        self.sources
            .iter()
            .filter_map(|(_, source)| match source {
                Kind::Tiled(source) => Some(source.attribution()),
                #[cfg(feature = "vector")]
                Kind::Untiled(source) => source.attribution(),
            })
            .collect()
    }

    /// Tiles being fetched or styled right now.
    pub fn stats(&self) -> Stats {
        let fetched: usize = self
            .sources
            .iter()
            .map(|(_, source)| match source {
                Kind::Tiled(source) => source.stats().in_progress,
                #[cfg(feature = "vector")]
                Kind::Untiled(_) => 0,
            })
            .sum();

        Stats {
            in_progress: fetched + self.styling.in_progress(),
        }
    }

    /// That tile, with the whole style drawn from the first source as one tile.
    pub(crate) fn single_source_at(&mut self, tile_id: TileId) -> Option<TilePiece> {
        let Self {
            style,
            sources,
            styling,
            ..
        } = self;
        let Some((_, Kind::Tiled(source))) = sources.first_mut() else {
            return None;
        };
        styling.at(0, source.as_mut(), 0..style.layer_sources().len(), tile_id)
    }
}

impl Layer for &mut StyledSources {
    fn draw(&mut self, canvas: &mut Canvas, projector: &Projector) {
        let StyledSources {
            style,
            sources,
            consecutive_layers,
            styling,
            ..
        } = &mut **self;
        let consecutive_layers =
            consecutive_layers.get_or_insert_with(|| consecutive_layers_of(style, sources));

        // One after another, so that a layer of one source can go between layers of another.
        for (index, layers) in consecutive_layers.iter() {
            match &mut sources[*index].1 {
                Kind::Tiled(source) => {
                    let source = source.as_mut();
                    let tile_size = source.tile_size();
                    draw_tiles_with(canvas, projector, tile_size, |tile_id| {
                        styling.at(*index, source, layers.clone(), tile_id)
                    });
                }
                #[cfg(feature = "vector")]
                Kind::Untiled(source) => source.draw(canvas, projector, style, layers.clone()),
            }
        }
    }
}

fn consecutive_layers_of(style: &Style, sources: &[NamedSource]) -> Vec<ConsecutiveLayers> {
    let names: Vec<&str> = sources.iter().map(|(name, _)| name.as_str()).collect();
    consecutive_layers(style, &names)
}

/// What a style layer makes of elevation. A tile is styled into either an image or shapes, so
/// layers making relief and those making contour lines are grouped apart.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(not(feature = "vector"), allow(dead_code))]
pub(crate) enum Drawn {
    Relief,
    Contours,
}

/// The style's layers, grouped into consecutive ones drawing from the same of `sources`, and
/// making the same of elevation. When there is only one source, the whole style draws from it.
fn consecutive_layers(style: &Style, sources: &[&str]) -> Vec<ConsecutiveLayers> {
    let layer_sources = style.layer_sources();
    let count = layer_sources.len();
    let single = matches!(sources, [_]);

    let mut consecutive: Vec<(usize, Range<usize>, Option<Drawn>)> = Vec::new();

    // Layers naming no source, such as a background, draw from the one before them. Those before
    // any other go with the first ones.
    let mut leading = None;

    for (index, (layer_source, drawn)) in layer_sources.enumerate() {
        let source = match layer_source {
            _ if single => 0,
            Some(name) => match sources.iter().position(|source| *source == name) {
                Some(source) => source,
                None => {
                    log::warn!(
                        "Style layer {index} draws from {name:?}, which is not among the sources. Skipping."
                    );
                    continue;
                }
            },
            None => match consecutive.last() {
                Some((source, _, _)) => *source,
                None => {
                    leading.get_or_insert(index);
                    continue;
                }
            },
        };

        match consecutive.last_mut() {
            Some((last, layers, last_drawn))
                if *last == source
                    && layers.end == index
                    && (last_drawn.is_none() || drawn.is_none() || *last_drawn == drawn) =>
            {
                layers.end += 1;
                *last_drawn = last_drawn.or(drawn);
            }
            _ => consecutive.push((source, leading.take().unwrap_or(index)..index + 1, drawn)),
        }
    }

    // Even with nothing to draw, as raster tiles are drawn without any layer.
    if single && consecutive.is_empty() {
        return vec![(0, 0..count)];
    }

    consecutive
        .into_iter()
        .map(|(source, layers, _)| (source, layers))
        .collect()
}

#[cfg(all(test, feature = "vector"))]
mod tests {
    use super::*;
    use crate::StyleLayer;

    fn raster(source: &str) -> StyleLayer {
        StyleLayer::Raster {
            source: Some(source.to_owned()),
            paint: Default::default(),
        }
    }

    fn style(layers: Vec<StyleLayer>) -> Style {
        Style {
            layers,
            ..Default::default()
        }
    }

    #[test]
    fn a_single_source_draws_every_layer_whatever_it_names() {
        let style = style(vec![raster("a"), raster("b"), StyleLayer::FillExtrusion]);
        assert_eq!(consecutive_layers(&style, &["only"]), [(0, 0..3)]);
    }

    /// Even with nothing to draw, as raster tiles are drawn without any layer.
    #[test]
    fn a_single_source_is_drawn_without_a_style() {
        assert_eq!(
            consecutive_layers(&style(Vec::new()), &["only"]),
            [(0, 0..0)]
        );
    }

    #[test]
    fn layers_of_one_source_between_those_of_another_split_it() {
        let style = style(vec![raster("a"), raster("a"), raster("b"), raster("a")]);
        assert_eq!(
            consecutive_layers(&style, &["a", "b"]),
            [(0, 0..2), (1, 2..3), (0, 3..4)]
        );
    }

    #[test]
    fn layers_of_unknown_sources_are_skipped() {
        let style = style(vec![raster("a"), raster("nope"), raster("a"), raster("b")]);
        assert_eq!(
            consecutive_layers(&style, &["a", "b"]),
            [(0, 0..1), (0, 2..3), (1, 3..4)]
        );
    }

    /// Background layers, as those of MapLibre's styles, name no source.
    #[test]
    fn layers_naming_no_source_draw_from_the_one_before_them() {
        let background = StyleLayer::Background {
            source: None,
            paint: Default::default(),
        };
        let style = style(vec![
            background,
            raster("a"),
            StyleLayer::FillExtrusion,
            raster("a"),
            raster("b"),
        ]);
        assert_eq!(
            consecutive_layers(&style, &["a", "b"]),
            [(0, 0..4), (1, 4..5)]
        );
    }

    fn hillshade(source: &str) -> StyleLayer {
        StyleLayer::Hillshade {
            source: Some(source.to_owned()),
            paint: Default::default(),
        }
    }

    fn contours(source: &str) -> StyleLayer {
        StyleLayer::Contour {
            source: Some(source.to_owned()),
            filter: None,
            layout: Default::default(),
            paint: None,
        }
    }

    #[test]
    fn relief_and_contour_lines_of_one_source_are_grouped_apart() {
        let style = style(vec![
            hillshade("terrain"),
            contours("terrain"),
            contours("terrain"),
            hillshade("terrain"),
        ]);
        let split = [(0, 0..1), (0, 1..3), (0, 3..4)];

        assert_eq!(consecutive_layers(&style, &["terrain", "other"]), split);
        assert_eq!(consecutive_layers(&style, &["terrain"]), split);
    }

    /// Such as a raster layer of a source the style has, but the map does not, in a style drawing
    /// vector tiles.
    #[test]
    fn other_layers_drawing_images_go_with_those_drawing_shapes() {
        let fill = || StyleLayer::Fill {
            source: Some("openmaptiles".to_owned()),
            source_layer: crate::SourceLayer::One("water".to_owned()),
            filter: None,
            paint: Default::default(),
        };
        let style = style(vec![fill(), raster("natural_earth"), fill()]);
        assert_eq!(consecutive_layers(&style, &["openmaptiles"]), [(0, 0..3)]);
    }

    #[test]
    fn layers_naming_no_source_go_with_whatever_is_drawn_around_them() {
        let background = || StyleLayer::Background {
            source: None,
            paint: Default::default(),
        };
        let style = style(vec![
            background(),
            hillshade("terrain"),
            background(),
            contours("terrain"),
        ]);

        assert_eq!(
            consecutive_layers(&style, &["terrain"]),
            [(0, 0..3), (0, 3..4)]
        );
    }

    #[test]
    fn no_sources_draw_nothing() {
        assert!(consecutive_layers(&style(vec![raster("a")]), &[]).is_empty());
    }

    /// Every tile is the same raster image, `size` pixels wide, so that sources can be told
    /// apart by what they draw.
    struct Fixed {
        png: bytes::Bytes,
        elevation: Option<crate::DemEncoding>,
    }

    impl Fixed {
        fn new(size: u32) -> Self {
            Self::of(image::RgbaImage::new(size, size))
        }

        fn of(image: image::RgbaImage) -> Self {
            let mut png = Vec::new();
            image
                .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                .unwrap();
            Self {
                png: png.into(),
                elevation: None,
            }
        }

        /// Ground rising to the south, Terrarium-encoded.
        fn elevation() -> Self {
            Self {
                elevation: Some(crate::DemEncoding::Terrarium),
                ..Self::of(crate::render::hillshade::tests::ramp(0.0, 20.0))
            }
        }
    }

    impl TiledSource for Fixed {
        fn data(&mut self, _tile_id: crate::TileId) -> crate::TileData {
            let data = self.png.clone();
            crate::TileData::Ready(Arc::new(match self.elevation {
                Some(encoding) => crate::SourceData::Elevation { data, encoding },
                None => crate::SourceData::Encoded(data),
            }))
        }

        fn attribution(&self) -> Attribution {
            Attribution {
                text: "",
                url: "",
                logo_light: None,
                logo_dark: None,
            }
        }
    }

    /// The map at `zoom`, 256 pixels wide, centered at `center`.
    fn projector_of(memory: &crate::MapMemory, center: crate::Position) -> Projector<'_> {
        Projector::new(
            emath::Rect::from_min_size(emath::Pos2::ZERO, emath::Vec2::splat(256.)),
            &crate::MercatorProjection,
            memory,
            center,
        )
    }

    fn memory_at(zoom: f64) -> crate::MapMemory {
        let mut memory = crate::MapMemory::default();
        memory.set_zoom(zoom).unwrap();
        memory
    }

    /// What `styled` puts on the canvas once it has `count` items, as tiles are styled in the
    /// background.
    fn draw_until(styled: &mut StyledSources, projector: &Projector, count: usize) -> Canvas {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let mut canvas = Canvas::new();
            (&mut *styled).draw(&mut canvas, projector);
            if canvas.items.len() == count {
                break canvas;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "tiles never got styled"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// Raster tiles drawn at zoom 0, where the whole world is one tile, so each group of layers
    /// draws exactly one.
    fn draw_rasters(styled: &mut StyledSources, count: usize) -> Vec<crate::canvas::RasterPiece> {
        let memory = memory_at(0.);
        let projector = projector_of(&memory, crate::lon_lat(0., 0.));

        draw_until(styled, &projector, count)
            .items
            .into_iter()
            .map(|item| match item {
                crate::canvas::Item::Raster(piece) => piece,
                _ => panic!("expected only raster tiles"),
            })
            .collect()
    }

    #[test]
    fn layers_are_drawn_in_their_order_whichever_source_they_draw_from() {
        let mut styled = StyledSources::new(
            style(vec![raster("a"), raster("b"), raster("a")]),
            Context::default(),
        )
        .with_tiled_source("a", Fixed::new(1))
        .with_tiled_source("b", Fixed::new(2));

        let sizes: Vec<usize> = draw_rasters(&mut styled, 3)
            .iter()
            .map(|piece| piece.image.0.size()[0])
            .collect();
        assert_eq!(sizes, [1, 2, 1]);
    }

    #[test]
    fn elevation_is_drawn_as_relief_by_hillshade_layers() {
        let style = serde_json::from_value(serde_json::json!({
            "layers": [
                { "type": "raster", "source": "a" },
                { "type": "hillshade", "source": "terrain" },
            ]
        }))
        .unwrap();
        let mut styled = StyledSources::new(style, Context::default())
            .with_tiled_source("a", Fixed::new(1))
            .with_tiled_source("terrain", Fixed::elevation());

        let sizes: Vec<usize> = draw_rasters(&mut styled, 2)
            .iter()
            .map(|piece| piece.image.0.size()[0])
            .collect();
        assert_eq!(sizes, [1, 16]);
    }

    fn line(source: &str, source_layer: &str) -> StyleLayer {
        serde_json::from_value(serde_json::json!({
            "type": "line",
            "source": source,
            "source-layer": source_layer,
            "paint": { "line-width": ["get", "width"] },
        }))
        .unwrap()
    }

    fn road(from: crate::Position, to: crate::Position, width: f64) -> crate::Feature {
        crate::Feature::new(geo_types::LineString::from(vec![from.0, to.0]))
            .with_property("width", width)
    }

    /// Lines drawn on the canvas, in screen coordinates, as their points and widths.
    fn lines_on(canvas: &Canvas) -> Vec<(Vec<emath::Pos2>, f32)> {
        canvas
            .items
            .iter()
            .flat_map(|item| match item {
                crate::canvas::Item::Drawables(drawables) => drawables
                    .drawables
                    .iter()
                    .flat_map(|drawable| match drawable {
                        crate::Drawable::Lines(lines) => lines
                            .iter()
                            .map(|line| {
                                let points = line
                                    .points
                                    .iter()
                                    .map(|point| drawables.transform * *point)
                                    .collect();
                                (points, line.width)
                            })
                            .collect(),
                        crate::Drawable::Fill(_) => Vec::new(),
                    })
                    .collect::<Vec<_>>(),
                crate::canvas::Item::Raster(_) => Vec::new(),
            })
            .collect()
    }

    fn assert_near(actual: &[emath::Pos2], expected: &[emath::Pos2]) {
        assert_eq!(actual.len(), expected.len(), "{actual:?}");
        for (actual, expected) in actual.iter().zip(expected) {
            assert!(
                actual.distance(*expected) < 0.01,
                "{actual:?} is not {expected:?}"
            );
        }
    }

    /// The equator, from 90° west to 90° east, is the middle half of the map's middle row at
    /// zoom 0.
    #[test]
    fn features_of_an_untiled_source_are_drawn_where_they_are_on_the_map() {
        let source = crate::MemorySource::new(vec![crate::FeatureLayer::new(
            "roads",
            vec![road(crate::lon_lat(-90., 0.), crate::lon_lat(90., 0.), 3.)],
        )]);
        let mut styled = StyledSources::new(style(vec![line("app", "roads")]), Context::default())
            .with_source("app", source);

        let memory = memory_at(0.);
        let canvas = draw_until(
            &mut styled,
            &projector_of(&memory, crate::lon_lat(0., 0.)),
            1,
        );

        let lines = lines_on(&canvas);
        assert_eq!(lines.len(), 1);
        assert_near(
            &lines[0].0,
            &[emath::pos2(64., 128.), emath::pos2(192., 128.)],
        );
        assert_eq!(lines[0].1, 3.);
    }

    #[test]
    fn layers_of_an_untiled_source_go_between_those_of_tiled_ones() {
        let source = crate::MemorySource::new(vec![crate::FeatureLayer::new(
            "roads",
            vec![road(crate::lon_lat(-90., 0.), crate::lon_lat(90., 0.), 3.)],
        )]);
        let mut styled = StyledSources::new(
            style(vec![raster("a"), line("app", "roads"), raster("a")]),
            Context::default(),
        )
        .with_tiled_source("a", Fixed::new(1))
        .with_source("app", source);

        let memory = memory_at(0.);
        let canvas = draw_until(
            &mut styled,
            &projector_of(&memory, crate::lon_lat(0., 0.)),
            3,
        );

        let kinds: Vec<&str> = canvas
            .items
            .iter()
            .map(|item| match item {
                crate::canvas::Item::Raster(_) => "raster",
                crate::canvas::Item::Drawables(_) => "drawables",
            })
            .collect();
        assert_eq!(kinds, ["raster", "drawables", "raster"]);
    }

    #[test]
    fn an_untiled_source_is_drawn_anew_when_its_features_change() {
        let source = crate::MemorySource::new(vec![crate::FeatureLayer::new(
            "roads",
            vec![road(crate::lon_lat(-90., 0.), crate::lon_lat(90., 0.), 3.)],
        )]);
        let mut styled = StyledSources::new(style(vec![line("app", "roads")]), Context::default())
            .with_source("app", source.clone());
        let memory = memory_at(0.);
        let projector = projector_of(&memory, crate::lon_lat(0., 0.));
        draw_until(&mut styled, &projector, 1);

        source.set(vec![crate::FeatureLayer::new(
            "roads",
            vec![road(crate::lon_lat(0., 0.), crate::lon_lat(90., 0.), 5.)],
        )]);

        let lines = lines_on(&draw_until(&mut styled, &projector, 1));
        assert_eq!(lines.len(), 1);
        assert_near(
            &lines[0].0,
            &[emath::pos2(128., 128.), emath::pos2(192., 128.)],
        );
        assert_eq!(lines[0].1, 5.);
    }

    /// Features are looked up by where they are, which must not change the order they are drawn
    /// in, each over the ones before it.
    #[test]
    fn features_of_an_untiled_source_are_drawn_in_their_order() {
        let roads = (1..=50)
            .map(|width| {
                let lat = (width as f64 * 7.3) % 60. - 30.;
                road(
                    crate::lon_lat(-90., lat),
                    crate::lon_lat(90., -lat),
                    width as f64,
                )
            })
            .collect();
        let source = crate::MemorySource::new(vec![crate::FeatureLayer::new("roads", roads)]);
        let mut styled = StyledSources::new(style(vec![line("app", "roads")]), Context::default())
            .with_source("app", source);

        let memory = memory_at(0.);
        let canvas = draw_until(
            &mut styled,
            &projector_of(&memory, crate::lon_lat(0., 0.)),
            1,
        );

        let widths: Vec<f32> = lines_on(&canvas).iter().map(|(_, width)| *width).collect();
        assert_eq!(
            widths,
            (1..=50).map(|width| width as f32).collect::<Vec<_>>()
        );
    }

    #[test]
    fn features_off_the_map_are_left_out() {
        let source = crate::MemorySource::new(vec![crate::FeatureLayer::new(
            "roads",
            vec![
                road(crate::lon_lat(17.0, 51.1), crate::lon_lat(17.1, 51.1), 1.),
                road(crate::lon_lat(-74.0, 40.7), crate::lon_lat(-73.9, 40.7), 2.),
            ],
        )]);
        let mut styled = StyledSources::new(style(vec![line("app", "roads")]), Context::default())
            .with_source("app", source);

        let memory = memory_at(10.);
        let canvas = draw_until(
            &mut styled,
            &projector_of(&memory, crate::lon_lat(17.05, 51.1)),
            1,
        );

        let widths: Vec<f32> = lines_on(&canvas).iter().map(|(_, width)| *width).collect();
        assert_eq!(widths, [1.]);
    }

    /// Labels of an untiled source are placed among the basemap's, by their style layer.
    #[test]
    fn labels_of_an_untiled_source_are_placed_by_their_style_layer() {
        let label = serde_json::from_value(serde_json::json!({
            "type": "symbol",
            "source": "app",
            "source-layer": "places",
            "layout": { "text-field": ["get", "name"] },
        }))
        .unwrap();
        let source = crate::MemorySource::new(vec![crate::FeatureLayer::new(
            "places",
            vec![
                crate::Feature::new(geo_types::Point::new(0., 0.))
                    .with_property("name", "Null Island"),
            ],
        )]);
        let mut styled = StyledSources::new(style(vec![raster("a"), label]), Context::default())
            .with_tiled_source("a", Fixed::new(1))
            .with_source("app", source);

        let memory = memory_at(0.);
        let canvas = draw_until(
            &mut styled,
            &projector_of(&memory, crate::lon_lat(0., 0.)),
            1,
        );

        let labels: Vec<_> = canvas
            .symbols
            .iter()
            .map(|(z_order, symbol)| {
                (
                    *z_order,
                    symbol.text.as_ref().map(|text| text.text.clone()),
                    symbol.position,
                )
            })
            .collect();
        assert_eq!(
            labels,
            [(
                crate::ZOrder::default().combine(1),
                Some("Null Island".to_string()),
                emath::pos2(128., 128.)
            )]
        );
    }
}
