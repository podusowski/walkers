[![crates.io](https://img.shields.io/crates/v/walkers.svg)](https://crates.io/crates/walkers)

Slippy maps widget for [egui](https://github.com/emilk/egui).

# Quick start

Walkers has three main objects. `Tiles` downloads images from a tile map provider
such as OpenStreetMap and stores them in a cache, `MapMemory` keeps track of
the widget's state and `Map` is the widget itself.

```rust
use walkers::{Tiles, Map, MapMemory, Position, openstreetmap};
use egui::{Context, CentralPanel};
use eframe::{App, Frame};

struct MyApp {
    tiles: Tiles,
    map_memory: MapMemory,
}

impl MyApp {
    fn new(egui_ctx: Context) -> Self {
        Self {
            tiles: Tiles::new(openstreetmap, egui_ctx),
            map_memory: MapMemory::default(),
        }
    }
}

impl App for MyApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        CentralPanel::default().show(ctx, |ui| {
            ui.add(Map::new(
                Some(&mut self.tiles),
                &mut self.map_memory,
                Position::new(17.03664, 51.09916)
            ));
        });
    }
}
```

# Limitations

There are couple of limitations when using this library. Some of them will
might probably be lifted at some point. Please raise an issue if you are
particularly affected by some and I will try to prioritize.

* ~~Limited to the OpenStreetMaps, but I want to enable other tile servers~~ and
  protocols (like WMS) as well.
* It uses `reqwests`/`tokio` stack which does not work on WASM.
* Example for Android is missing, but it does work there.

Other suggestions are welcomed as well.

![Screenshot](https://raw.githubusercontent.com/podusowski/walkers/main/screenshot.png)
