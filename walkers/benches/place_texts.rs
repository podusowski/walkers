//! Times placing the labels of a screenful of dense vector tiles: laying them out and dropping
//! those which collide with, or repeat, an already placed one.
//!
//! ```text
//! cargo bench -p walkers --features mvt --bench place_texts
//! ```
//!
//! The tile is Wrocław at zoom 14 in the OpenMapTiles schema (see `decode_mvt.rs`), repeated
//! over a grid, as the map shows several tiles at once.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use egui::vec2;
use walkers::{Palette, Symbol, Tile, Waymark, place_symbols};

const OPENMAPTILES_TILE: &[u8] = include_bytes!("wroclaw-14-8967-5477-openmaptiles.mvt");
const ZOOM: u8 = 14;
const TILE_SIZE: u32 = 512;

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    // Fonts only exist once a pass has been run.
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
    output.textures_delta.clear();
    ctx
}

/// The tile's labels, repeated over `n` × `n` tiles.
fn labels(n: usize) -> Vec<Symbol> {
    let style = Waymark::new(Palette::dark()).build();
    let Tile::Vector { symbols, .. } =
        Tile::from_mvt(OPENMAPTILES_TILE, &style, ZOOM, TILE_SIZE).unwrap()
    else {
        panic!("not a vector tile");
    };

    (0..n * n)
        .flat_map(|k| {
            let offset = vec2((k % n) as f32, (k / n) as f32) * TILE_SIZE as f32;
            symbols.iter().map(move |symbol| Symbol {
                position: symbol.position + offset,
                ..symbol.clone()
            })
        })
        .collect()
}

fn bench_place_texts(c: &mut Criterion) {
    let ctx = context();
    let mut group = c.benchmark_group("place_texts");

    for n in [2, 4] {
        let texts = labels(n);
        group.bench_function(format!("{n}x{n} tiles, {} labels", texts.len()), |b| {
            b.iter(|| place_symbols(black_box(texts.clone()), &ctx))
        });
    }

    group.finish();
}

criterion_group!(benches, bench_place_texts);
criterion_main!(benches);
