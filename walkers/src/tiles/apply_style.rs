//! Decoding a tile's data, and applying style layers to it. This is the one place which tells what
//! each kind of style layer draws, and from which data.

use std::ops::Range;

use egui::ColorImage;
use image::{ImageReader, RgbaImage};

use super::TileError;
#[cfg(feature = "vector")]
use super::heights::Heights;
use crate::{SourceData, TileId, style::Style};

#[cfg(feature = "vector")]
use crate::{
    Drawable,
    expression::Context,
    render::{self, Geometry},
    style::{Filter, Layout, Paint, SourceLayer, StyleLayer},
    symbol::Symbol,
};
#[cfg(feature = "vector")]
use std::collections::HashMap;

/// A tile's data, decoded.
pub(crate) enum Decoded {
    /// Features, grouped in named layers. Only MVTs have them, for now.
    #[cfg(feature = "vector")]
    #[cfg_attr(not(feature = "mvt"), allow(dead_code))]
    Features(Vec<FeatureLayer>),
    Image(RgbaImage),
    #[cfg(feature = "vector")]
    Elevation(Heights),
}

/// Features of a layer of a vector tile, in the tile's pixels.
#[cfg(feature = "vector")]
pub(crate) struct FeatureLayer {
    pub name: String,
    pub features: Vec<(Geometry<f32>, Context)>,
}

/// What style layers make of a tile.
pub(crate) enum Styled {
    #[cfg(feature = "vector")]
    Vector {
        drawables: Vec<Drawable>,
        symbols: Vec<Symbol>,
    },
    Image(ColorImage),
}

/// What `layers` of the style make of `data`, the data of the tile `tile_id`, at `zoom`. That is
/// the zoom the tile is drawn at, deeper than the tile's own where it stands in for tiles past
/// the last zoom level of its source.
pub(crate) fn apply_style(
    data: &SourceData,
    style: &Style,
    layers: Range<usize>,
    tile_id: TileId,
    zoom: u8,
    tile_size: u32,
) -> Result<Styled, TileError> {
    let decoded = decode(data, style, layers.clone(), zoom, tile_size)?;
    apply(decoded, style, layers, tile_id, zoom, tile_size)
}

fn decode(
    data: &SourceData,
    style: &Style,
    layers: Range<usize>,
    zoom: u8,
    tile_size: u32,
) -> Result<Decoded, TileError> {
    match data {
        SourceData::Encoded(bytes) => decode_encoded(bytes, style, layers, zoom, tile_size),
        #[cfg(feature = "vector")]
        SourceData::Elevation { data, encoding } => {
            Ok(Decoded::Elevation(Heights::decode(data, *encoding)?))
        }
    }
}

/// A raster image or an MVT, told apart by the data. Only the MVT layers which `layers` draw
/// from are decoded.
fn decode_encoded(
    data: &[u8],
    style: &Style,
    layers: Range<usize>,
    zoom: u8,
    tile_size: u32,
) -> Result<Decoded, TileError> {
    if data.is_empty() {
        return Err(TileError::Empty);
    }

    let reader = ImageReader::new(std::io::Cursor::new(data)).with_guessed_format()?;
    if reader.format().is_some() {
        log::debug!("Decoding tile as raster image.");
        return Ok(Decoded::Image(reader.decode()?.to_rgba8()));
    }

    #[cfg(feature = "mvt")]
    {
        log::debug!("Trying to decode tile as MVT vector tile.");
        let needed = |name: &str| draws_from(style, layers.clone(), name);
        Ok(Decoded::Features(super::mvt::decode(
            data, needed, zoom, tile_size,
        )?))
    }

    #[cfg(not(feature = "mvt"))]
    {
        let _ = (style, layers, zoom, tile_size);
        Err(TileError::UnrecognizedFormat)
    }
}

/// Whether any of `layers` draws features of the vector tile layer named `name`.
#[cfg(feature = "mvt")]
fn draws_from(style: &Style, layers: Range<usize>, name: &str) -> bool {
    style.layers[layers].iter().any(|layer| match layer {
        StyleLayer::Fill { source_layer, .. }
        | StyleLayer::Line { source_layer, .. }
        | StyleLayer::Symbol { source_layer, .. } => source_layer.matches(name),
        // Circle layers name a source layer, but are not drawn.
        _ => false,
    })
}

#[cfg(feature = "vector")]
fn apply(
    decoded: Decoded,
    style: &Style,
    layers: Range<usize>,
    tile_id: TileId,
    zoom: u8,
    tile_size: u32,
) -> Result<Styled, TileError> {
    let mut drawables = Vec::new();
    let mut symbols = Vec::new();
    let mut hillshades = Vec::new();
    let mut raster_opacity = None;

    // Traced by the first contour layer, for all of them.
    let mut contours = None;

    // Tessellated for the first symbol layer drawing an icon, for all of them.
    let mut icons = None;
    let no_icons = HashMap::new();

    // Indices stay those of the whole style, as they decide which labels win.
    for (index, layer) in layers.clone().zip(&style.layers[layers]) {
        match (layer, &decoded) {
            (StyleLayer::Background { paint, .. }, Decoded::Features(_)) => {
                drawables.push(background(paint, zoom, tile_size));
            }
            (
                StyleLayer::Fill {
                    source_layer,
                    filter,
                    paint,
                    ..
                },
                Decoded::Features(features),
            ) => {
                for (geometry, context) in matching(features, source_layer, filter.as_ref()) {
                    warn_on_error(render::fill::render(
                        geometry,
                        context,
                        paint,
                        &mut drawables,
                    ));
                }
            }
            (
                StyleLayer::Line {
                    source_layer,
                    filter,
                    paint,
                    ..
                },
                Decoded::Features(features),
            ) => {
                for (geometry, context) in matching(features, source_layer, filter.as_ref()) {
                    warn_on_error(render::line::render(
                        geometry,
                        context,
                        paint,
                        &mut drawables,
                    ));
                }
            }
            (
                StyleLayer::Symbol {
                    source_layer,
                    minzoom,
                    filter,
                    layout,
                    paint,
                    ..
                },
                Decoded::Features(features),
            ) => {
                let icons = match layout.icon_image {
                    Some(_) => &*icons.get_or_insert_with(|| {
                        render::symbol::tessellate_icons(&style.icons, zoom)
                    }),
                    None => &no_icons,
                };

                for (geometry, context) in matching(features, source_layer, filter.as_ref()) {
                    symbols.extend(render::symbol::render(
                        geometry, context, layout, paint, icons, *minzoom, index,
                    ));
                }
            }
            (StyleLayer::Circle { .. } | StyleLayer::FillExtrusion, Decoded::Features(_)) => {
                log::warn!("Unsupported layer type in style: {layer:?}");
            }
            (StyleLayer::Raster { paint, .. }, Decoded::Image(_)) => {
                raster_opacity.get_or_insert(opacity(paint, zoom));
            }
            (StyleLayer::Hillshade { paint, .. }, Decoded::Elevation(_)) => {
                hillshades.push(paint);
            }
            (
                StyleLayer::Contour {
                    filter,
                    layout,
                    paint,
                    ..
                },
                Decoded::Elevation(heights),
            ) => {
                let lines = contours.get_or_insert_with(|| {
                    super::contours::features(heights, tile_id.zoom, zoom, tile_size)
                });
                draw_contours(
                    lines,
                    filter.as_ref(),
                    layout,
                    paint,
                    index,
                    &mut drawables,
                    &mut symbols,
                );
            }
            // Layers of other kinds of data, which a single source draws every layer of.
            _ => {}
        }
    }

    match decoded {
        Decoded::Features(_) => Ok(Styled::Vector {
            drawables: render::merge_runs(drawables),
            symbols,
        }),
        Decoded::Image(image) => match raster_opacity {
            Some(opacity) => Ok(Styled::Image(color_image(&image, opacity))),
            None => Err(TileError::Unstyled(
                "only a raster style layer draws an image",
            )),
        },
        // Hillshade and contour layers are styled apart, so it is never both.
        Decoded::Elevation(heights) if !hillshades.is_empty() => {
            let size = [heights.width(), heights.height()];
            let pixels = render::hillshade::render(&heights, &hillshades, tile_id, zoom);
            Ok(Styled::Image(ColorImage::new(size, pixels)))
        }
        Decoded::Elevation(_) if contours.is_some() => Ok(Styled::Vector {
            drawables: render::merge_runs(drawables),
            symbols,
        }),
        Decoded::Elevation(_) => Err(UNDRAWN_ELEVATION),
    }
}

/// Contour `lines` which `filter` lets through, drawn and labelled as a contour layer at `index`
/// says.
#[cfg(feature = "vector")]
fn draw_contours(
    lines: &[(Geometry<f32>, Context)],
    filter: Option<&Filter>,
    layout: &Layout,
    paint: &Option<Paint>,
    index: usize,
    drawables: &mut Vec<Drawable>,
    symbols: &mut Vec<Symbol>,
) {
    let default = Paint::default();
    let line_paint = paint.as_ref().unwrap_or(&default);

    for (geometry, context) in lines
        .iter()
        .filter(|(_, context)| filter.is_none_or(|filter| filter.matches(context)))
    {
        // Leaving only the labels.
        let hidden = line_paint
            .line_opacity
            .as_ref()
            .is_some_and(|opacity| opacity.evaluate(context) <= 0.0);
        if !hidden {
            warn_on_error(render::line::render(
                geometry, context, line_paint, drawables,
            ));
        }

        if layout.text_field.is_some() {
            symbols.extend(render::symbol::render(
                geometry,
                context,
                layout,
                paint,
                &HashMap::new(),
                None,
                index,
            ));
        }
    }
}

#[cfg(feature = "vector")]
const UNDRAWN_ELEVATION: TileError =
    TileError::Unstyled("elevation is drawn only by hillshade and contour style layers");

/// Without the `vector` feature, a style has no layers, and images are drawn as they are.
#[cfg(not(feature = "vector"))]
fn apply(
    decoded: Decoded,
    _style: &Style,
    _layers: Range<usize>,
    _tile_id: TileId,
    _zoom: u8,
    _tile_size: u32,
) -> Result<Styled, TileError> {
    match decoded {
        Decoded::Image(image) => Ok(Styled::Image(color_image(&image, 1.0))),
    }
}

fn color_image(image: &RgbaImage, opacity: f32) -> ColorImage {
    let mut color_image = ColorImage::from_rgba_unmultiplied(
        [image.width() as _, image.height() as _],
        image.as_flat_samples().as_slice(),
    );
    if opacity < 1.0 {
        for pixel in &mut color_image.pixels {
            *pixel = pixel.gamma_multiply(opacity);
        }
    }
    color_image
}

/// <https://maplibre.org/maplibre-style-spec/layers/#raster-opacity>
#[cfg(feature = "vector")]
fn opacity(paint: &Paint, zoom: u8) -> f32 {
    let context = Context::new("None".to_string(), Default::default(), zoom);
    paint
        .raster_opacity
        .as_ref()
        .map_or(1.0, |opacity| opacity.evaluate(&context))
}

#[cfg(feature = "vector")]
fn background(paint: &Paint, zoom: u8, tile_size: u32) -> Drawable {
    let context = Context::new("None".to_string(), Default::default(), zoom);
    let color = paint
        .background_color
        .as_ref()
        .map_or(ecolor::Color32::WHITE, |color| color.evaluate(&context));
    Drawable::background(tile_size as f32, color)
}

/// Features of the layers `source_layer` names, which `filter` lets through.
#[cfg(feature = "vector")]
fn matching<'a>(
    layers: &'a [FeatureLayer],
    source_layer: &'a SourceLayer,
    filter: Option<&'a Filter>,
) -> impl Iterator<Item = &'a (Geometry<f32>, Context)> {
    layers
        .iter()
        .filter(|layer| source_layer.matches(&layer.name))
        .flat_map(|layer| &layer.features)
        .filter(move |(_, context)| filter.is_none_or(|filter| filter.matches(context)))
}

#[cfg(feature = "vector")]
fn warn_on_error(result: Result<(), render::Error>) {
    if let Err(err) = result {
        log::warn!("{err}");
    }
}

#[cfg(all(test, feature = "vector"))]
mod tests {
    use super::*;
    use crate::{DemEncoding, render::hillshade::tests::ramp};

    fn style(layers: serde_json::Value) -> Style {
        serde_json::from_value(serde_json::json!({ "layers": layers })).unwrap()
    }

    const TILE: TileId = TileId {
        x: 0,
        y: 0,
        zoom: 0,
    };

    fn elevation() -> Decoded {
        Decoded::Elevation(Heights::from_image(
            &ramp(0.0, 20.0),
            DemEncoding::Terrarium,
        ))
    }

    #[test]
    fn elevation_is_shaded_by_hillshade_layers() {
        let style = style(serde_json::json!([{ "type": "hillshade" }]));
        assert!(matches!(
            apply(elevation(), &style, 0..1, TILE, TILE.zoom, 256),
            Ok(Styled::Image(_))
        ));
    }

    /// Its pixels are not colors, so drawing them as they are would make no sense.
    #[test]
    fn elevation_is_not_drawn_without_a_hillshade_layer() {
        let style = style(serde_json::json!([{ "type": "raster" }]));
        assert!(apply(elevation(), &style, 0..1, TILE, TILE.zoom, 256).is_err());
    }

    /// Not even by a hillshade layer, as its colors are not packed elevation, which only a source
    /// made `with_elevation` has.
    #[test]
    fn image_is_drawn_only_by_raster_layers() {
        let image = || Decoded::Image(ramp(0.0, 0.0));
        let hillshade = style(serde_json::json!([{ "type": "hillshade" }]));

        assert!(apply(image(), &hillshade, 0..1, TILE, TILE.zoom, 256).is_err());
        assert!(
            apply(
                image(),
                &Style {
                    layers: Vec::new(),
                    ..Default::default()
                },
                0..0,
                TILE,
                TILE.zoom,
                256
            )
            .is_err()
        );
        assert!(apply(image(), &Style::default(), 0..1, TILE, TILE.zoom, 256).is_ok());
    }

    fn elevation_data() -> SourceData {
        let mut png = Vec::new();
        ramp(0.0, 20.0)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        SourceData::Elevation {
            data: png.into(),
            encoding: DemEncoding::Terrarium,
        }
    }

    const ZOOMED_IN: TileId = TileId {
        x: 4454,
        y: 2751,
        zoom: 13,
    };

    /// How many lines and labels `layers` draw of elevation rising from 990 to 1310 meters.
    fn contoured(layers: serde_json::Value) -> Result<(usize, usize), TileError> {
        let style = style(layers);
        match apply_style(
            &elevation_data(),
            &style,
            0..style.layers.len(),
            ZOOMED_IN,
            ZOOMED_IN.zoom,
            256,
        )? {
            Styled::Vector { drawables, symbols } => {
                let lines = drawables
                    .iter()
                    .map(|drawable| match drawable {
                        Drawable::Lines(lines) => lines.len(),
                        Drawable::Fill(_) => 0,
                    })
                    .sum();
                Ok((lines, symbols.len()))
            }
            Styled::Image(_) => panic!("expected lines"),
        }
    }

    #[test]
    fn contour_layers_draw_contour_lines_of_elevation() {
        let drawn = contoured(serde_json::json!([
            { "type": "contour", "paint": { "line-color": "red" } }
        ]));
        // 1000 to 1300 meters, every 20.
        assert_eq!(drawn.unwrap(), (16, 0));
    }

    #[test]
    fn major_contour_lines_are_told_apart() {
        let drawn = contoured(serde_json::json!([
            { "type": "contour", "filter": ["==", ["get", "level"], 1] }
        ]));
        // Every 100 meters.
        assert_eq!(drawn.unwrap(), (4, 0));
    }

    #[test]
    fn contour_lines_are_labelled_with_a_text_field() {
        let drawn = contoured(serde_json::json!([
            {
                "type": "contour",
                "layout": { "text-field": ["to-string", ["get", "ele"]] }
            }
        ]));
        assert_eq!(drawn.unwrap(), (16, 16));
    }

    #[test]
    fn contour_lines_which_are_not_opaque_at_all_leave_only_labels() {
        let drawn = contoured(serde_json::json!([
            { "type": "contour", "paint": { "line-color": "red" } },
            {
                "type": "contour",
                "filter": ["==", ["get", "level"], 1],
                "layout": { "text-field": ["to-string", ["get", "ele"]] },
                "paint": { "line-opacity": 0 }
            }
        ]));
        assert_eq!(drawn.unwrap(), (16, 4));
    }

    /// Only a contour layer makes lines of elevation, whatever source layer a line layer names.
    #[test]
    fn line_layers_draw_nothing_of_elevation() {
        let drawn = contoured(serde_json::json!([
            { "type": "line", "source-layer": "contours", "paint": {} }
        ]));
        assert!(drawn.is_err());
    }

    #[test]
    fn raster_layers_draw_images_with_their_opacity() {
        let style = style(serde_json::json!([
            { "type": "raster", "paint": { "raster-opacity": 0.25 } }
        ]));

        let Ok(Styled::Image(image)) = apply(
            Decoded::Image(ramp(0.0, 0.0)),
            &style,
            0..1,
            TILE,
            TILE.zoom,
            256,
        ) else {
            panic!("expected an image");
        };
        assert!(image.pixels.iter().all(|pixel| pixel.a() == 64));
    }

    /// A point of the `mountain_peak` layer, put through a symbol layer of `layout`, in a style
    /// with a `peak` icon.
    fn symbols_drawn(layout: serde_json::Value) -> Vec<Symbol> {
        let style: Style = serde_json::from_value(serde_json::json!({
            "icons": {
                "peak": [{ "fill": [[0, -5], [5, 5], [-5, 5]], "fill-color": "#ff0000" }]
            },
            "layers": [{
                "type": "symbol",
                "source-layer": "mountain_peak",
                "layout": layout
            }]
        }))
        .unwrap();

        let features = Decoded::Features(vec![FeatureLayer {
            name: "mountain_peak".into(),
            features: vec![(
                Geometry::Point(geo_types::Point::new(10., 20.)),
                Context::new("Point".into(), Default::default(), 12),
            )],
        }]);

        match apply(features, &style, 0..1, TILE, 12, 256) {
            Ok(Styled::Vector { symbols, .. }) => symbols,
            _ => panic!("expected drawables"),
        }
    }

    #[test]
    fn symbol_layers_put_the_icons_they_name_on_points() {
        let symbols = symbols_drawn(serde_json::json!({ "icon-image": "peak" }));

        match symbols.as_slice() {
            [
                Symbol {
                    text: None,
                    icon: Some(icon),
                    position,
                    ..
                },
            ] => {
                assert_eq!(*position, emath::pos2(10., 20.));
                match icon.drawables.as_slice() {
                    [Drawable::Fill(mesh)] => {
                        assert_eq!(mesh.indices.len(), 3);
                        assert!(
                            mesh.vertices
                                .iter()
                                .all(|vertex| vertex.color == ecolor::Color32::RED)
                        );
                    }
                    other => panic!("expected a single fill, got {other:?}"),
                }
            }
            other => panic!("expected a single icon, got {other:?}"),
        }
    }

    #[test]
    fn an_icon_and_a_text_of_a_layer_make_one_symbol() {
        let symbols = symbols_drawn(serde_json::json!({
            "icon-image": "peak",
            "text-field": "Śnieżka"
        }));

        assert!(matches!(
            symbols.as_slice(),
            [Symbol {
                text: Some(_),
                icon: Some(_),
                ..
            }]
        ));
    }

    #[test]
    fn an_icon_the_style_does_not_have_is_not_drawn() {
        assert!(symbols_drawn(serde_json::json!({ "icon-image": "volcano" })).is_empty());
    }
}
