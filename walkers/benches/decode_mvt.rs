//! Times decoding a single dense vector tile.
//!
//! ```text
//! cargo bench -p walkers --features mvt --bench decode_mvt
//! ```
//!
//! The tiles are Wrocław at zoom 14. One is in the Protomaps basemap schema, extracted from a
//! Protomaps PMTiles build of Poland with:
//!
//! ```text
//! pmtiles tile poland.pmtiles 14 8967 5477 | gunzip > wroclaw-14-8967-5477.mvt
//! ```
//!
//! The other is in the OpenMapTiles schema, extracted from an OpenFreeMap planet build with:
//!
//! ```text
//! pmtiles tile https://btrfs.openfreemap.com/areas/planet/20261101_080013_pt/tiles.pmtiles \
//!     14 8967 5477 | gunzip > wroclaw-14-8967-5477-openmaptiles.mvt
//! ```

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use walkers::{Style, Tile};

const TILE: &[u8] = include_bytes!("wroclaw-14-8967-5477.mvt");
const OPENMAPTILES_TILE: &[u8] = include_bytes!("wroclaw-14-8967-5477-openmaptiles.mvt");
const ZOOM: u8 = 14;
const TILE_SIZE: u32 = 1024;

fn decode_mvt(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_mvt");

    group.bench_function("protomaps light", |b| {
        let style = Style::protomaps_light();
        b.iter(|| Tile::from_mvt(black_box(TILE), &style, ZOOM, TILE_SIZE).unwrap())
    });

    group.bench_function("openmaptiles light", |b| {
        let style = Style::openmaptiles_basemap_light();
        b.iter(|| Tile::from_mvt(black_box(OPENMAPTILES_TILE), &style, ZOOM, TILE_SIZE).unwrap())
    });

    group.bench_function("no style", |b| {
        let style = Style { layers: Vec::new() };
        b.iter(|| Tile::from_mvt(black_box(TILE), &style, ZOOM, TILE_SIZE).unwrap())
    });

    group.finish();
}

criterion_group!(benches, decode_mvt);
criterion_main!(benches);
