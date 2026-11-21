//! Contour lines, traced through the elevation of a tile.

use std::collections::{BTreeMap, HashMap, VecDeque};

use geo::Simplify;
use geo_types::{Coord, LineString, MultiLineString};
use serde_json::json;

use super::heights::{Grid, Heights};
use crate::{expression::Context, render::Geometry};

/// How far a simplified line may stray from the traced one, in pixels as drawn.
const TOLERANCE: f32 = 0.5;

/// Closed lines shorter than that, in pixels as drawn, are left out.
const SHORTEST_RING: f32 = 12.0;

/// Meters between contour lines, and between the major ones among them, at `zoom`. None below
/// zoom 9.
fn intervals(zoom: u8) -> Option<(i32, i32)> {
    match zoom {
        ..=8 => None,
        9 | 10 => Some((200, 1000)),
        11 => Some((100, 500)),
        12 => Some((50, 250)),
        13 => Some((20, 100)),
        14 | 15 => Some((10, 50)),
        16.. => Some((5, 25)),
    }
}

/// Contour lines through `heights`, of a tile of `data_zoom` drawn at `zoom`, in pixels of a tile
/// `tile_size` wide.
///
/// Each elevation is one feature, with `ele` in meters, and `level` being 1 for major lines and 0
/// for the others.
pub(crate) fn features(
    heights: &Heights,
    data_zoom: u8,
    zoom: u8,
    tile_size: u32,
) -> Vec<(Geometry<f32>, Context)> {
    match (intervals(zoom), Grid::new(heights)) {
        (Some((minor, major)), Some(grid)) => {
            let scale = Coord {
                x: tile_size as f32 / heights.width() as f32,
                y: tile_size as f32 / heights.height() as f32,
            };

            // Past the last zoom level of the source, its tile is drawn magnified.
            let magnified = 2f32.powi(zoom.saturating_sub(data_zoom).into());

            trace(&grid, minor as f32, scale)
                .into_iter()
                .filter_map(|(step, lines)| {
                    let lines: Vec<_> = lines
                        .into_iter()
                        .map(|line| line.simplify(TOLERANCE / magnified))
                        .filter(|line| !is_small_ring(line, SHORTEST_RING / magnified))
                        .collect();
                    if lines.is_empty() {
                        return None;
                    }

                    let ele = step * minor;
                    let properties = HashMap::from([
                        ("ele".to_owned(), json!(ele)),
                        ("level".to_owned(), json!(i32::from(ele % major == 0))),
                    ]);
                    Some((
                        Geometry::MultiLineString(MultiLineString(lines)),
                        Context::new("LineString".to_owned(), properties, zoom),
                    ))
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

fn is_small_ring(line: &LineString<f32>, shortest: f32) -> bool {
    line.is_closed()
        && line
            .lines()
            .map(|segment| segment.dx().hypot(segment.dy()))
            .sum::<f32>()
            < shortest
}

/// An edge of a cell of the grid, as the corner it starts at, and whether it goes right rather
/// than down.
type Edge = (u32, u32, bool);

/// Where a line crosses a cell, from one of its edges to another.
struct Segment {
    from: (Edge, Coord<f32>),
    to: (Edge, Coord<f32>),
}

/// Lines where the ground is at each multiple of `interval`, by how many intervals that is, in
/// pixels of the tile times `scale`. Marching squares.
fn trace(grid: &Grid, interval: f32, scale: Coord<f32>) -> BTreeMap<i32, Vec<LineString<f32>>> {
    let mut segments: BTreeMap<i32, Vec<Segment>> = BTreeMap::new();

    for y in 0..grid.ys().len() - 1 {
        for x in 0..grid.xs().len() - 1 {
            // Clockwise from the top left.
            let corners = [
                grid.at(x, y),
                grid.at(x + 1, y),
                grid.at(x + 1, y + 1),
                grid.at(x, y + 1),
            ];
            let lowest = corners.into_iter().fold(f32::INFINITY, f32::min);
            let highest = corners.into_iter().fold(f32::NEG_INFINITY, f32::max);

            for step in (lowest / interval).ceil() as i32..=(highest / interval).floor() as i32 {
                let cell = Cell {
                    x,
                    y,
                    corners,
                    level: step as f32 * interval,
                    grid,
                    scale,
                };
                segments.entry(step).or_default().extend(cell.segments());
            }
        }
    }

    segments
        .into_iter()
        .map(|(step, segments)| (step, joined(segments)))
        .collect()
}

struct Cell<'a> {
    x: usize,
    y: usize,
    corners: [f32; 4],
    level: f32,
    grid: &'a Grid,
    scale: Coord<f32>,
}

impl Cell<'_> {
    /// Pieces of the line at `level` crossing this cell.
    fn segments(&self) -> impl Iterator<Item = Segment> + '_ {
        let above = self.corners.map(|corner| corner > self.level);
        let case = above.iter().enumerate().fold(0, |case, (index, above)| {
            case | (usize::from(*above) << index)
        });

        // A saddle, crossed one way or another depending on the middle of the cell.
        let middle_above = self.corners.iter().sum::<f32>() / 4.0 > self.level;

        // Edges, clockwise from the top: 0 top, 1 right, 2 bottom, 3 left.
        let pairs: &[(u8, u8)] = match case {
            0 | 15 => &[],
            1 | 14 => &[(3, 0)],
            2 | 13 => &[(0, 1)],
            3 | 12 => &[(3, 1)],
            4 | 11 => &[(1, 2)],
            6 | 9 => &[(0, 2)],
            7 | 8 => &[(3, 2)],
            5 if middle_above => &[(3, 2), (0, 1)],
            5 => &[(3, 0), (1, 2)],
            10 if middle_above => &[(3, 0), (1, 2)],
            _ => &[(0, 1), (3, 2)],
        };

        pairs.iter().map(|&(from, to)| Segment {
            from: self.crossing(from),
            to: self.crossing(to),
        })
    }

    /// Where the line crosses that edge of the cell.
    fn crossing(&self, edge: u8) -> (Edge, Coord<f32>) {
        let (x, y) = (self.x, self.y);
        let [top_left, top_right, bottom_right, bottom_left] = self.corners;
        let between = |a: f32, b: f32| (self.level - a) / (b - a);
        let along = |positions: &[f32], at: usize, fraction: f32| {
            positions[at] + (positions[at + 1] - positions[at]) * fraction
        };
        let (xs, ys) = (self.grid.xs(), self.grid.ys());
        let (cx, cy) = (x as u32, y as u32);

        let (edge, at) = match edge {
            0 => (
                (cx, cy, true),
                Coord {
                    x: along(xs, x, between(top_left, top_right)),
                    y: ys[y],
                },
            ),
            1 => (
                (cx + 1, cy, false),
                Coord {
                    x: xs[x + 1],
                    y: along(ys, y, between(top_right, bottom_right)),
                },
            ),
            2 => (
                (cx, cy + 1, true),
                Coord {
                    x: along(xs, x, between(bottom_left, bottom_right)),
                    y: ys[y + 1],
                },
            ),
            _ => (
                (cx, cy, false),
                Coord {
                    x: xs[x],
                    y: along(ys, y, between(top_left, bottom_left)),
                },
            ),
        };

        (
            edge,
            Coord {
                x: at.x * self.scale.x,
                y: at.y * self.scale.y,
            },
        )
    }
}

/// `segments` of one level, joined into lines where they share an edge.
fn joined(segments: Vec<Segment>) -> Vec<LineString<f32>> {
    // Two segments at most cross an edge, one of each cell next to it.
    let mut at_edge: HashMap<Edge, (usize, Option<usize>)> =
        HashMap::with_capacity(segments.len() * 2);
    for (index, segment) in segments.iter().enumerate() {
        for edge in [segment.from.0, segment.to.0] {
            at_edge
                .entry(edge)
                .and_modify(|(_, other)| *other = Some(index))
                .or_insert((index, None));
        }
    }

    let mut used = vec![false; segments.len()];
    let mut lines = Vec::new();

    for start in 0..segments.len() {
        if used[start] {
            continue;
        }
        used[start] = true;

        let mut points = VecDeque::from([segments[start].from.1, segments[start].to.1]);

        // On from its end, then back from its start.
        for forward in [true, false] {
            let mut edge = if forward {
                segments[start].to.0
            } else {
                segments[start].from.0
            };

            loop {
                let (one, other) = at_edge[&edge];
                let Some(next) = [Some(one), other].into_iter().flatten().find(|i| !used[*i])
                else {
                    break;
                };
                used[next] = true;

                let segment = &segments[next];
                let (far_edge, point) = if segment.from.0 == edge {
                    segment.to
                } else {
                    segment.from
                };
                if forward {
                    points.push_back(point);
                } else {
                    points.push_front(point);
                }
                edge = far_edge;
            }
        }

        lines.push(LineString(points.into()));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DemEncoding, tiles::heights::tests::terrarium};
    use approx::assert_relative_eq;
    use image::RgbaImage;

    /// 16 by 16 pixels of ground `at` each pixel's center.
    fn ground(at: impl Fn(f32, f32) -> f32) -> Heights {
        let image =
            RgbaImage::from_fn(16, 16, |x, y| terrarium(at(x as f32 + 0.5, y as f32 + 0.5)));
        Heights::from_image(&image, DemEncoding::Terrarium)
    }

    /// Rising by 10 meters with every pixel to the south, from 1002 meters at the north edge.
    fn slope() -> Heights {
        ground(|_, y| 1002.0 + 10.0 * y)
    }

    fn lines(layer: &[(Geometry<f32>, Context)]) -> Vec<(i64, i64, &MultiLineString<f32>)> {
        layer
            .iter()
            .map(|(geometry, context)| {
                let Geometry::MultiLineString(lines) = geometry else {
                    panic!("expected lines, got {geometry:?}");
                };
                let property = |name: &str| {
                    context
                        .evaluate(&json!(["get", name]))
                        .unwrap()
                        .as_i64()
                        .unwrap()
                };
                (property("ele"), property("level"), lines)
            })
            .collect()
    }

    #[test]
    fn lines_are_drawn_where_the_ground_is_at_each_interval() {
        let layer = features(&slope(), 13, 13, 16);

        let lines = lines(&layer);
        let elevations: Vec<i64> = lines.iter().map(|(ele, _, _)| *ele).collect();
        assert_eq!(elevations, (1020..=1160).step_by(20).collect::<Vec<_>>());

        for (ele, _, line) in lines {
            let [line] = &line.0[..] else {
                panic!("expected a single line at {ele}");
            };
            // Straight, so simplified into its two ends.
            let [from, to] = line.0[..] else {
                panic!("expected a straight line at {ele}, got {line:?}");
            };
            let y = (ele as f32 - 1002.0) / 10.0;
            assert_relative_eq!(from.y, y, epsilon = 1e-3);
            assert_relative_eq!(to.y, y, epsilon = 1e-3);
        }
    }

    /// So that they meet the lines of the tiles around, rather than stop half a pixel short.
    #[test]
    fn lines_reach_the_edges_of_the_tile() {
        let layer = features(&slope(), 13, 13, 16);

        for (_, _, line) in lines(&layer) {
            let xs: Vec<f32> = line.0[0].0.iter().map(|point| point.x).collect();
            assert_eq!(xs.iter().copied().fold(f32::INFINITY, f32::min), 0.0);
            assert_eq!(xs.iter().copied().fold(f32::NEG_INFINITY, f32::max), 16.0);
        }
    }

    #[test]
    fn lines_are_in_pixels_of_the_tile() {
        let layer = features(&slope(), 13, 13, 512);

        for (ele, _, line) in lines(&layer) {
            let y = (ele as f32 - 1002.0) / 10.0 * 32.0;
            assert!(line.0[0].0.iter().all(|point| (point.y - y).abs() < 1e-2));
            assert!(line.0[0].0.iter().any(|point| point.x == 512.0));
        }
    }

    #[test]
    fn major_lines_are_told_apart() {
        let layer = features(&slope(), 13, 13, 16);

        let majors: Vec<i64> = lines(&layer)
            .into_iter()
            .filter(|(_, level, _)| *level == 1)
            .map(|(ele, _, _)| ele)
            .collect();
        assert_eq!(majors, [1100]);
    }

    #[test]
    fn lines_get_closer_with_the_zoom_they_are_drawn_at() {
        let count = |zoom| lines(&features(&slope(), 13, zoom, 16)).len();
        assert_eq!(count(12), 3);
        assert_eq!(count(13), 8);
        assert_eq!(count(16), 32);
    }

    #[test]
    fn there_are_none_when_zoomed_out() {
        let layer = features(&slope(), 8, 8, 16);
        assert!(layer.is_empty());
    }

    #[test]
    fn hill_is_ringed_by_closed_lines() {
        let hill = ground(|x, y| 1090.0 - 4.0 * (x - 8.0).hypot(y - 8.0));
        let layer = features(&hill, 13, 13, 256);

        let lines = lines(&layer);
        let elevations: Vec<i64> = lines.iter().map(|(ele, _, _)| *ele).collect();
        assert_eq!(elevations, [1060, 1080]);
        for (ele, _, line) in lines {
            assert!(
                line.0.iter().all(LineString::is_closed),
                "line at {ele} is not closed"
            );
        }
    }

    #[test]
    fn bumps_too_small_to_make_out_are_left_out() {
        let bump = ground(|x, y| if x == 8.5 && y == 8.5 { 1030.0 } else { 1001.0 });
        let layer = features(&bump, 13, 13, 16);
        assert!(layer.is_empty());
    }

    #[test]
    fn saddle_is_crossed_by_two_lines() {
        let corners = [1030.0, 1010.0, 1010.0, 1030.0];
        let image = RgbaImage::from_fn(2, 2, |x, y| terrarium(corners[(y * 2 + x) as usize]));
        let grid = Grid::new(&Heights::from_image(&image, DemEncoding::Terrarium)).unwrap();

        // Kept apart, as the middle of the cell is no higher than the line.
        let traced = trace(&grid, 20.0, Coord { x: 1.0, y: 1.0 });
        assert_eq!(traced[&51].len(), 2);
    }
}
