use std::{ops::Range, sync::Arc};

use egui::Context;

use crate::{
    Canvas, Layer, Projector, Stats, Style, TilePiece, TiledSource,
    sources::Attribution,
    tiles::{EguiTileFactory, TileId, draw_tiles_with, styling::Styling},
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

    /// `None` until they are needed, as they change with every source added.
    runs: Option<Vec<Run>>,

    egui_ctx: Context,
}

type NamedSource = (String, Box<dyn TiledSource>);

/// Consecutive layers of the style drawing from the same source.
struct Run {
    /// Index of the source.
    source: usize,
    styling: Styling,
}

impl StyledSources {
    pub fn new(style: Style, egui_ctx: Context) -> Self {
        Self {
            style: Arc::new(style),
            sources: Vec::new(),
            runs: None,
            egui_ctx,
        }
    }

    /// Add a source, which the style's layers can name to draw from it.
    pub fn with_tiled_source(
        mut self,
        name: impl Into<String>,
        source: impl TiledSource + 'static,
    ) -> Self {
        self.sources.push((name.into(), Box::new(source)));
        self.runs = None;
        self
    }

    /// Draw with another style from now on. The sources keep what they have fetched, so that
    /// nothing is fetched again.
    pub fn set_style(&mut self, style: Style) {
        self.style = Arc::new(style);
        self.runs = None;
    }

    /// Of every source.
    pub fn attributions(&self) -> Vec<Attribution> {
        self.sources
            .iter()
            .map(|(_, source)| source.attribution())
            .collect()
    }

    /// Tiles being fetched or styled right now.
    pub fn stats(&self) -> Stats {
        let fetched: usize = self
            .sources
            .iter()
            .map(|(_, source)| source.stats().in_progress)
            .sum();
        let styled: usize = self
            .runs
            .iter()
            .flatten()
            .map(|run| run.styling.in_progress())
            .sum();

        Stats {
            in_progress: fetched + styled,
        }
    }

    /// That tile of the first run, for those holding just one source, which is drawn in one run.
    pub(crate) fn first_run_at(&mut self, tile_id: TileId) -> Option<TilePiece> {
        let (runs, sources) = self.runs_and_sources();
        let run = runs.first_mut()?;
        run.styling.at(sources[run.source].1.as_mut(), tile_id)
    }

    /// The runs, made when first needed, and the sources they draw from.
    fn runs_and_sources(&mut self) -> (&mut [Run], &mut [NamedSource]) {
        let runs = self.runs.get_or_insert_with(|| {
            let names: Vec<&str> = self.sources.iter().map(|(name, _)| name.as_str()).collect();
            runs(&self.style, &names)
                .into_iter()
                .map(|(source, layers)| Run {
                    source,
                    styling: Styling::new(
                        EguiTileFactory::new(
                            self.egui_ctx.clone(),
                            self.style.clone(),
                            layers,
                            self.sources[source].1.tile_size(),
                        ),
                        self.egui_ctx.clone(),
                    ),
                })
                .collect()
        });
        (runs, &mut self.sources)
    }
}

impl Layer for &mut StyledSources {
    fn draw(&mut self, canvas: &mut Canvas, projector: &Projector) {
        let (runs, sources) = self.runs_and_sources();

        // Run after run, so that a layer of one source can go between layers of another.
        for run in runs {
            let source = sources[run.source].1.as_mut();
            let tile_size = source.tile_size();
            draw_tiles_with(canvas, projector, tile_size, |tile_id| {
                run.styling.at(source, tile_id)
            });
        }
    }
}

/// Runs of consecutive layers drawing from the same source, as the index of the source and the
/// layers. When there is only one source, the whole style draws from it.
fn runs(style: &Style, sources: &[&str]) -> Vec<(usize, Range<usize>)> {
    #[cfg(feature = "vector")]
    let layers = style.layers.len();
    #[cfg(not(feature = "vector"))]
    let layers = {
        let _ = style;
        0
    };

    if let [_] = sources {
        return vec![(0, 0..layers)];
    }

    #[cfg(not(feature = "vector"))]
    return Vec::new();

    #[cfg(feature = "vector")]
    {
        let mut runs: Vec<(usize, Range<usize>)> = Vec::new();

        // Layers naming no source, such as a background, draw from the one before them. Those
        // before any other go into the first run.
        let mut leading = None;

        for (index, layer) in style.layers.iter().enumerate() {
            let source = match layer.source() {
                Some(name) => match sources.iter().position(|source| *source == name) {
                    Some(source) => source,
                    None => {
                        log::warn!(
                            "Style layer {index} draws from {name:?}, which is not among the sources. Skipping."
                        );
                        continue;
                    }
                },
                None => match runs.last() {
                    Some((source, _)) => *source,
                    None => {
                        leading.get_or_insert(index);
                        continue;
                    }
                },
            };

            match runs.last_mut() {
                Some((last, layers)) if *last == source && layers.end == index => layers.end += 1,
                _ => runs.push((source, leading.take().unwrap_or(index)..index + 1)),
            }
        }

        runs
    }
}

#[cfg(all(test, feature = "vector"))]
mod tests {
    use super::*;
    use crate::StyleLayer;

    fn raster(source: &str) -> StyleLayer {
        StyleLayer::Raster {
            source: Some(source.to_owned()),
        }
    }

    fn style(layers: Vec<StyleLayer>) -> Style {
        Style { layers }
    }

    #[test]
    fn a_single_source_draws_every_layer_whatever_it_names() {
        let style = style(vec![raster("a"), raster("b"), StyleLayer::FillExtrusion]);
        assert_eq!(runs(&style, &["only"]), [(0, 0..3)]);
    }

    /// Even with nothing to draw, as raster tiles are drawn without any layer.
    #[test]
    fn a_single_source_is_drawn_without_a_style() {
        assert_eq!(runs(&style(Vec::new()), &["only"]), [(0, 0..0)]);
    }

    #[test]
    fn layers_of_one_source_between_those_of_another_split_it() {
        let style = style(vec![raster("a"), raster("a"), raster("b"), raster("a")]);
        assert_eq!(runs(&style, &["a", "b"]), [(0, 0..2), (1, 2..3), (0, 3..4)]);
    }

    #[test]
    fn layers_of_unknown_sources_are_skipped() {
        let style = style(vec![raster("a"), raster("nope"), raster("a"), raster("b")]);
        assert_eq!(runs(&style, &["a", "b"]), [(0, 0..1), (0, 2..3), (1, 3..4)]);
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
        assert_eq!(runs(&style, &["a", "b"]), [(0, 0..4), (1, 4..5)]);
    }

    #[test]
    fn no_sources_draw_nothing() {
        assert!(runs(&style(vec![raster("a")]), &[]).is_empty());
    }

    /// Every tile is the same raster image, `size` pixels wide, so that sources can be told
    /// apart by what they draw.
    struct Fixed {
        png: bytes::Bytes,
    }

    impl Fixed {
        fn new(size: u32) -> Self {
            let mut png = Vec::new();
            image::RgbaImage::new(size, size)
                .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                .unwrap();
            Self { png: png.into() }
        }
    }

    impl TiledSource for Fixed {
        fn data(&mut self, _tile_id: crate::TileId) -> Option<Arc<crate::SourceData>> {
            Some(Arc::new(crate::SourceData::Encoded(self.png.clone())))
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

    #[test]
    fn layers_are_drawn_in_their_order_whichever_source_they_draw_from() {
        let ctx = Context::default();
        let mut styled = StyledSources::new(
            style(vec![raster("a"), raster("b"), raster("a")]),
            ctx.clone(),
        )
        .with_tiled_source("a", Fixed::new(1))
        .with_tiled_source("b", Fixed::new(2));

        // The whole world is one tile at zoom 0, so each layer draws exactly one.
        let mut memory = crate::MapMemory::default();
        memory.set_zoom(0.).unwrap();
        let projector = Projector::new(
            emath::Rect::from_min_size(emath::Pos2::ZERO, emath::Vec2::splat(256.)),
            &crate::MercatorProjection,
            &memory,
            crate::lon_lat(0., 0.),
        );

        // Tiles are styled in the background.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let drawn = loop {
            let mut canvas = Canvas::new();
            (&mut styled).draw(&mut canvas, &projector);
            if canvas.items.len() == 3 {
                break canvas.items;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "tiles never got styled"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        };

        let sizes: Vec<usize> = drawn
            .iter()
            .map(|item| match item {
                crate::canvas::Item::Raster(piece) => piece.image.0.size()[0],
                _ => panic!("expected only raster tiles"),
            })
            .collect();
        assert_eq!(sizes, [1, 2, 1]);
    }
}
