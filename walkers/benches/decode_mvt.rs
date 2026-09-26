//! Times decoding a single dense vector tile.
//!
//! ```text
//! cargo bench -p walkers --features mvt --bench decode_mvt
//! ```
//!
//! The tile is Wrocław at zoom 14 in the Protomaps basemap schema, extracted from a Protomaps
//! PMTiles build of Poland with:
//!
//! ```text
//! pmtiles tile poland.pmtiles 14 8967 5477 | gunzip > wroclaw-14-8967-5477.mvt
//! ```

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use walkers::{Style, Tile};

const TILE: &[u8] = include_bytes!("wroclaw-14-8967-5477.mvt");
const ZOOM: u8 = 14;
const TILE_SIZE: u32 = 1024;

fn decode_mvt(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_mvt");

    group.bench_function("protomaps light", |b| {
        let style = Style::protomaps_basemap_light();
        b.iter(|| Tile::from_mvt(black_box(TILE), &style, ZOOM, TILE_SIZE).unwrap())
    });

    group.bench_function("no style", |b| {
        let style = Style::default();
        b.iter(|| Tile::from_mvt(black_box(TILE), &style, ZOOM, TILE_SIZE).unwrap())
    });

    group.finish();
}

criterion_group!(benches, decode_mvt);
criterion_main!(benches);
