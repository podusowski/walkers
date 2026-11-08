use std::{ops::Range, sync::Arc};

use egui::Context;

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

type NamedSource = (String, Box<dyn TiledSource>);

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
        self.sources.push((name.into(), Box::new(source)));
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

        Stats {
            in_progress: fetched + self.styling.in_progress(),
        }
    }

    /// That tile, for those holding a single source, which draws the whole style.
    pub(crate) fn single_source_at(&mut self, tile_id: TileId) -> Option<TilePiece> {
        let Self {
            style,
            sources,
            consecutive_layers,
            styling,
            ..
        } = self;
        let (source, layers) = consecutive_layers
            .get_or_insert_with(|| consecutive_layers_of(style, sources))
            .first()?;
        styling.at(
            *source,
            sources[*source].1.as_mut(),
            layers.clone(),
            tile_id,
        )
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
            let source = sources[*index].1.as_mut();
            let tile_size = source.tile_size();
            draw_tiles_with(canvas, projector, tile_size, |tile_id| {
                styling.at(*index, source, layers.clone(), tile_id)
            });
        }
    }
}

fn consecutive_layers_of(style: &Style, sources: &[NamedSource]) -> Vec<ConsecutiveLayers> {
    let names: Vec<&str> = sources.iter().map(|(name, _)| name.as_str()).collect();
    consecutive_layers(style, &names)
}

/// The style's layers, grouped into consecutive ones drawing from the same of `sources`. When there
/// is only one source, the whole style draws from it.
fn consecutive_layers(style: &Style, sources: &[&str]) -> Vec<ConsecutiveLayers> {
    let layer_sources = style.layer_sources();

    if let [_] = sources {
        return vec![(0, 0..layer_sources.len())];
    }

    let mut consecutive: Vec<ConsecutiveLayers> = Vec::new();

    // Layers naming no source, such as a background, draw from the one before them. Those before
    // any other go with the first ones.
    let mut leading = None;

    for (index, layer_source) in layer_sources.enumerate() {
        let source = match layer_source {
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
                Some((source, _)) => *source,
                None => {
                    leading.get_or_insert(index);
                    continue;
                }
            },
        };

        match consecutive.last_mut() {
            Some((last, layers)) if *last == source && layers.end == index => layers.end += 1,
            _ => consecutive.push((source, leading.take().unwrap_or(index)..index + 1)),
        }
    }

    consecutive
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

    #[test]
    fn no_sources_draw_nothing() {
        assert!(consecutive_layers(&style(vec![raster("a")]), &[]).is_empty());
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
