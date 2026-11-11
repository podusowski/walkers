//! Decoding a tile's data, and applying style layers to it. This is the one place which tells what
//! each kind of style layer draws, and from which data.

use std::ops::Range;

use egui::ColorImage;
use image::{ImageReader, RgbaImage};

use super::TileError;
use crate::{DemEncoding, SourceData, TileId, style::Style};

#[cfg(feature = "vector")]
use crate::{
    Drawable,
    expression::Context,
    render::{self, Geometry},
    style::{Filter, Paint, SourceLayer, StyleLayer},
    text::Text,
};

/// A tile's data, decoded.
pub(crate) enum Decoded {
    /// Features, grouped in named layers. Only MVTs have them, for now.
    #[cfg(feature = "vector")]
    #[cfg_attr(not(feature = "mvt"), allow(dead_code))]
    Features(Vec<FeatureLayer>),
    Image(RgbaImage),
    #[cfg_attr(not(feature = "vector"), allow(dead_code))]
    Elevation {
        image: RgbaImage,
        encoding: DemEncoding,
    },
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
        texts: Vec<Text>,
    },
    Image(ColorImage),
}

/// What `layers` of the style make of `data`, the data of the tile `tile_id`.
pub(crate) fn apply_style(
    data: &SourceData,
    style: &Style,
    layers: Range<usize>,
    tile_id: TileId,
    tile_size: u32,
) -> Result<Styled, TileError> {
    let decoded = decode(data, style, layers.clone(), tile_id.zoom, tile_size)?;
    apply(decoded, style, layers, tile_id, tile_size)
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
        SourceData::Elevation { data, encoding } => Ok(Decoded::Elevation {
            image: image::load_from_memory(data)?.to_rgba8(),
            encoding: *encoding,
        }),
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
    tile_size: u32,
) -> Result<Styled, TileError> {
    let zoom = tile_id.zoom;
    let mut drawables = Vec::new();
    let mut texts = Vec::new();
    let mut hillshades = Vec::new();
    let mut raster = false;
    let mut shaded = false;

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
                for (geometry, context) in matching(features, source_layer, filter.as_ref()) {
                    warn_on_error(render::symbol::render(
                        geometry, context, &mut texts, layout, paint, *minzoom, index,
                    ));
                }
            }
            (StyleLayer::Circle { .. } | StyleLayer::FillExtrusion, Decoded::Features(_)) => {
                log::warn!("Unsupported layer type in style: {layer:?}");
            }
            (StyleLayer::Raster { .. }, Decoded::Image(_)) => raster = true,
            (StyleLayer::Hillshade { .. }, Decoded::Image(_)) => shaded = true,
            (StyleLayer::Hillshade { paint, .. }, Decoded::Elevation { .. }) => {
                hillshades.push(paint);
            }
            // Layers of other kinds of data, which a single source draws every layer of.
            _ => {}
        }
    }

    match decoded {
        Decoded::Features(_) => Ok(Styled::Vector {
            drawables: render::merge_runs(drawables),
            texts,
        }),
        // Its colors would be packed elevation.
        Decoded::Image(_) if shaded && !raster => Err(TileError::Unstyled(
            "a hillshade style layer draws only from a source made `with_elevation`",
        )),
        // Even without a raster layer, so that raster tiles need no style.
        Decoded::Image(image) => Ok(Styled::Image(color_image(&image))),
        Decoded::Elevation { .. } if hillshades.is_empty() => Err(UNSHADED),
        Decoded::Elevation { image, encoding } => {
            let size = [image.width() as usize, image.height() as usize];
            let pixels = render::hillshade::render(&image, encoding, &hillshades, tile_id);
            Ok(Styled::Image(ColorImage::new(size, pixels)))
        }
    }
}

const UNSHADED: TileError =
    TileError::Unstyled("elevation is drawn only by a hillshade style layer");

/// Without the `vector` feature, a style has no layers, and images are drawn as they are.
#[cfg(not(feature = "vector"))]
fn apply(
    decoded: Decoded,
    _style: &Style,
    _layers: Range<usize>,
    _tile_id: TileId,
    _tile_size: u32,
) -> Result<Styled, TileError> {
    match decoded {
        Decoded::Image(image) => Ok(Styled::Image(color_image(&image))),
        Decoded::Elevation { .. } => Err(UNSHADED),
    }
}

fn color_image(image: &RgbaImage) -> ColorImage {
    ColorImage::from_rgba_unmultiplied(
        [image.width() as _, image.height() as _],
        image.as_flat_samples().as_slice(),
    )
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
    use crate::render::hillshade::tests::ramp;

    fn style(layers: serde_json::Value) -> Style {
        serde_json::from_value(serde_json::json!({ "layers": layers })).unwrap()
    }

    const TILE: TileId = TileId {
        x: 0,
        y: 0,
        zoom: 0,
    };

    fn elevation() -> Decoded {
        Decoded::Elevation {
            image: ramp(0.0, 20.0),
            encoding: DemEncoding::Terrarium,
        }
    }

    #[test]
    fn elevation_is_shaded_by_hillshade_layers() {
        let style = style(serde_json::json!([{ "type": "hillshade" }]));
        assert!(matches!(
            apply(elevation(), &style, 0..1, TILE, 256),
            Ok(Styled::Image(_))
        ));
    }

    /// Its pixels are not colors, so drawing them as they are would make no sense.
    #[test]
    fn elevation_is_not_drawn_without_a_hillshade_layer() {
        let style = style(serde_json::json!([{ "type": "raster" }]));
        assert!(apply(elevation(), &style, 0..1, TILE, 256).is_err());
    }

    /// Its colors would be taken for packed elevation, which only a source made
    /// `with_elevation` has.
    #[test]
    fn image_is_not_drawn_by_hillshade_layers_alone() {
        let style = style(serde_json::json!([{ "type": "hillshade" }]));
        assert!(apply(Decoded::Image(ramp(0.0, 20.0)), &style, 0..1, TILE, 256).is_err());
    }

    /// So that raster tile servers need no style.
    #[test]
    fn image_is_drawn_as_it_is_without_layers() {
        assert!(matches!(
            apply(
                Decoded::Image(ramp(0.0, 0.0)),
                &Style::default(),
                0..0,
                TILE,
                256
            ),
            Ok(Styled::Image(_))
        ));
    }
}
