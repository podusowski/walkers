use std::collections::HashMap;

use ecolor::Color32;
use emath::pos2;
use log::warn;
use lyon_path::geom::point;

use crate::{
    Drawable, Drawables, Mesh,
    expression::Context,
    render::{Geometry, Line, fill::tessellate_polygon},
    style::{IconFill, Layout, Paint},
    symbol::{Placement, Symbol, Text},
};

/// What a symbol layer of `layout` and `paint` puts on the geometry: a text, the icon it names
/// out of `icons`, or both. Icons are put only on points.
pub fn render(
    geometry: &Geometry<f32>,
    context: &Context,
    layout: &Layout,
    paint: &Option<Paint>,
    icons: &HashMap<String, Drawables>,
    min_zoom: Option<f32>,
    layer: usize,
) -> Vec<Symbol> {
    let mut symbols = match geometry {
        Geometry::Point(point) => {
            label_points(std::slice::from_ref(point), context, layout, paint, icons)
        }
        Geometry::MultiPoint(multi_point) => {
            label_points(&multi_point.0, context, layout, paint, icons)
        }
        Geometry::LineString(line_string) => {
            label_line_strings(std::slice::from_ref(line_string), context, layout, paint)
        }
        Geometry::MultiLineString(multi_line_string) => {
            label_line_strings(&multi_line_string.0, context, layout, paint)
        }
        _ => Vec::new(),
    };

    let sort_key = layout.symbol_sort_key(context);
    for symbol in &mut symbols {
        symbol.min_zoom = min_zoom;
        symbol.layer = layer;
        symbol.sort_key = sort_key;
    }
    symbols
}

/// Icons of a style, by name, their colors as at `zoom`. An icon which cannot be tessellated is
/// left out.
pub(crate) fn tessellate_icons(
    icons: &HashMap<String, Vec<IconFill>>,
    zoom: u8,
) -> HashMap<String, Drawables> {
    let context = Context::new("None".to_string(), Default::default(), zoom);

    icons
        .iter()
        .filter_map(|(name, fills)| {
            let mut mesh = Mesh::default();
            for fill in fills {
                let points: Vec<_> = fill.fill.iter().map(|[x, y]| point(*x, *y)).collect();
                match tessellate_polygon(&points, &[], fill.fill_color.evaluate(&context)) {
                    Ok(fill) => mesh.append(&fill),
                    Err(err) => {
                        warn!("Icon '{name}' could not be tessellated: {err}");
                        return None;
                    }
                }
            }
            Some((name.clone(), Drawables::new(vec![Drawable::Fill(mesh)])))
        })
        .collect()
}

/// Put labels and icons on top of points.
fn label_points(
    points: &[geo_types::Point<f32>],
    context: &Context,
    layout: &Layout,
    paint: &Option<Paint>,
    icons: &HashMap<String, Drawables>,
) -> Vec<Symbol> {
    let text = layout.text(context).map(|text| {
        let text_size = evaluate_text_size(layout, context);
        let (halo_color, halo_width) = evaluate_halo(paint, context);
        Text::new(text, text_size, evaluate_text_color(paint, context), 0.0)
            .with_halo(halo_color, halo_width)
            .with_padding(evaluate_text_padding(layout, context))
            .with_anchor(layout.anchor(context), layout.offset(context) * text_size)
    });
    let icon = layout.icon_image(context).and_then(|name| icons.get(&name));

    if text.is_none() && icon.is_none() {
        return Vec::new();
    }

    points
        .iter()
        .map(|p| Symbol {
            text: text.clone(),
            icon: icon.cloned(),
            ..Symbol::new(pos2(p.x(), p.y()))
        })
        .collect()
}

/// Put labels on top of lines.
fn label_line_strings(
    line_strings: &[geo_types::LineString<f32>],
    context: &Context,
    layout: &Layout,
    paint: &Option<Paint>,
) -> Vec<Symbol> {
    let Some(text) = layout.text(context) else {
        return Vec::new();
    };

    let text_size = evaluate_text_size(layout, context);
    let text_color = evaluate_text_color(paint, context);
    let (halo_color, halo_width) = evaluate_halo(paint, context);
    let padding = evaluate_text_padding(layout, context);

    line_strings
        .iter()
        .filter_map(|line_string| {
            // Use the longest line to fit the label.
            let line = line_string.lines().max_by_key(|line| length(line) as u32)?;
            let mid_point = midpoint(&line.start_point(), &line.end_point());
            let angle = line.slope().atan();

            Some(
                Symbol::new(pos2(mid_point.x(), mid_point.y())).with_text(
                    Text::new(text.clone(), text_size, text_color, angle)
                        .with_halo(halo_color, halo_width)
                        .with_placement(Placement::Line)
                        .with_max_width(Some(length(&line)))
                        .with_padding(padding),
                ),
            )
        })
        .collect()
}

fn evaluate_text_padding(layout: &Layout, context: &Context) -> f32 {
    layout
        .text_padding
        .as_ref()
        .map_or(0.0, |padding| padding.evaluate(context))
}

fn evaluate_text_size(layout: &Layout, context: &Context) -> f32 {
    layout
        .text_size
        .as_ref()
        .and_then(|text_size| {
            let size = text_size.evaluate(context);

            if size > 3.0 {
                Some(size)
            } else {
                warn!(
                    "{} evaluated into {size}, which is too small for text size.",
                    text_size.0
                );
                None
            }
        })
        // Default from MapLibre spec.
        .unwrap_or(12.0)
}

/// A halo is only drawn where the style asks for one; the spec has no width by default.
fn evaluate_halo(paint: &Option<Paint>, context: &Context) -> (Color32, f32) {
    let Some(paint) = paint else {
        return (Color32::TRANSPARENT, 0.0);
    };

    let color = match &paint.text_halo_color {
        Some(color) => color.evaluate(context),
        None => return (Color32::TRANSPARENT, 0.0),
    };

    let width = match &paint.text_halo_width {
        Some(width) => width.evaluate(context),
        None => 0.0,
    };

    (color, width)
}

fn evaluate_text_color(paint: &Option<Paint>, context: &Context) -> Color32 {
    if let Some(paint) = paint
        && let Some(color) = &paint.text_color
    {
        color.evaluate(context)
    } else {
        // Default from MapLibre spec.
        Color32::BLACK
    }
}

fn length(line: &Line<f32>) -> f32 {
    (line.dx() * line.dx() + line.dy() * line.dy()).sqrt()
}

fn midpoint(p1: &geo_types::Point<f32>, p2: &geo_types::Point<f32>) -> geo_types::Point<f32> {
    geo_types::Point::new((p1.x() + p2.x()) / 2.0, (p1.y() + p2.y()) / 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{geometry_type_to_str, transformed_symbols};
    use crate::symbol::place_symbols;
    use emath::TSTransform;
    use std::collections::HashMap;
    fn label(geometry: Geometry<f32>) -> Vec<Symbol> {
        let context = Context::new(
            geometry_type_to_str(&geometry).to_string(),
            HashMap::from([("name".to_string(), serde_json::Value::from("Śnieżka"))]),
            12,
        );

        let layout = Layout {
            text_field: Some(crate::style::json!(["get", "name"])),
            ..Default::default()
        };

        render(
            &geometry,
            &context,
            &layout,
            &None,
            &HashMap::new(),
            None,
            0,
        )
    }

    fn texts(symbols: &[Symbol]) -> Vec<&str> {
        symbols
            .iter()
            .filter_map(|symbol| Some(symbol.text.as_ref()?.text.as_str()))
            .collect()
    }

    /// MVT hands over `MultiPoint`, GeoJSON a plain `Point`, and both need labelling.
    #[test]
    fn points_are_labelled_whether_they_come_singly_or_not() {
        let point = geo_types::Point::new(1.0, 2.0);

        assert_eq!(texts(&label(Geometry::Point(point))), ["Śnieżka"]);
        assert_eq!(
            texts(&label(Geometry::MultiPoint(
                vec![point, geo_types::Point::new(3.0, 4.0)].into()
            ))),
            ["Śnieżka", "Śnieżka"]
        );
    }

    #[test]
    fn line_strings_are_labelled_whether_they_come_singly_or_not() {
        let line_string =
            geo_types::LineString::from(vec![(0.0f32, 0.0f32), (10.0, 0.0), (10.0, 10.0)]);

        assert_eq!(
            texts(&label(Geometry::LineString(line_string.clone()))),
            ["Śnieżka"]
        );
        assert_eq!(
            texts(&label(Geometry::MultiLineString(
                geo_types::MultiLineString::new(vec![line_string.clone(), line_string])
            ))),
            ["Śnieżka", "Śnieżka"]
        );
    }

    /// A street name is only drawn once the street, as seen on the screen, is long enough to
    /// hold it, the label keeping its size while the map is zoomed.
    #[test]
    fn a_street_name_waits_until_the_street_is_long_enough_to_hold_it() {
        let ctx = egui::Context::default();
        // Fonts only exist once a pass has been run.
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();

        let line_string =
            geo_types::LineString::from(vec![(0.0f32, 0.0f32), (10.0, 0.0), (10.0, 30.0)]);
        let symbols = label(Geometry::LineString(line_string));

        let placed = |scaling: f32| {
            let symbols = transformed_symbols(&symbols, TSTransform::from_scaling(scaling), 0.);
            place_symbols(symbols, &ctx).len()
        };

        assert_eq!(placed(1.), 0);
        assert_eq!(placed(10.), 1);
    }

    /// The offset is in the label's own font size, as `text-offset` is.
    #[test]
    fn a_point_label_takes_its_anchor_and_offset_from_the_style() {
        let point = Geometry::Point(geo_types::Point::new(1.0, 2.0));
        let context = Context::new("Point".to_string(), HashMap::new(), 12);
        let layout: Layout = serde_json::from_value(crate::style::json!({
            "text-field": "Ślęża",
            "text-size": 10,
            "text-anchor": "top",
            "text-offset": [0, 1.5]
        }))
        .unwrap();

        let symbols = render(&point, &context, &layout, &None, &HashMap::new(), None, 0);

        match symbols.as_slice() {
            [
                Symbol {
                    text: Some(text), ..
                },
            ] => {
                assert_eq!(text.anchor, crate::symbol::Anchor::Top);
                assert_eq!(text.offset, emath::vec2(0.0, 15.0));
            }
            other => panic!("expected a single label, got {other:?}"),
        }
    }

    /// Labels are placed on the geometry, not at the origin.
    #[test]
    fn a_point_label_lands_on_the_point() {
        let symbols = label(Geometry::Point(geo_types::Point::new(1.0, 2.0)));

        match symbols.as_slice() {
            [symbol] => assert_eq!(symbol.position, pos2(1.0, 2.0)),
            other => panic!("expected a single label, got {other:?}"),
        }
    }
}
