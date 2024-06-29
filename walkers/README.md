# Walkers, a map widget for Rust

[![crates.io](https://img.shields.io/crates/v/walkers.svg)](https://crates.io/crates/walkers)
[![docs.rs](https://img.shields.io/docsrs/walkers/latest)](https://docs.rs/walkers/latest/)

Walkers is a slippy maps widget for [egui](https://github.com/emilk/egui), 
similar to very popular [Leaflet](https://leafletjs.com/), but written in Rust.
It compiles to native applications as well as WASM. See the **[online demo here](https://podusowski.github.io/walkers/)**.

![Screenshot](https://raw.githubusercontent.com/podusowski/walkers/main/screenshot.png)

It supports [OpenStreetMap](https://www.openstreetmap.org), [mapbox](https://www.mapbox.com/), 
and compatible tile servers.

Before deploying your application, please get yourself familiar with the
[OpenStreetMap usage policy](https://operations.osmfoundation.org/policies/tiles/), 
and consider donating the [OpenStreetMap Foundation](https://supporting.openstreetmap.org/).

## Quick start

Walkers has three main objects. `Tiles` downloads images from a tile map provider
such as OpenStreetMap and stores them in a cache, `MapMemory` keeps track of
the widget's state and `Map` is the widget itself.

```rust
use walkers::{HttpTiles, Map, MapMemory, Position, sources::OpenStreetMap};
use egui::{Context, CentralPanel};
use eframe::{App, Frame};

struct MyApp {
    tiles: HttpTiles,
    map_memory: MapMemory,
}

impl MyApp {
    fn new(egui_ctx: Context) -> Self {
        Self {
            tiles: HttpTiles::new(OpenStreetMap, egui_ctx),
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
                Position::from_lon_lat(17.03664, 51.09916)
            ));
        });
    }
}
```

You can see a more complete example [here](https://github.com/podusowski/walkers/blob/main/demo/src/lib.rs).

## Running demos

Walkers suports numerous build options, such as Android and WASM. They all share
a common library - `demo`, but require a different build workflow, not
necessarily compatible with Cargo alone.

### Native

To enable mapbox layers, you need to define `MAPBOX_ACCESS_TOKEN` environment
variable before building. You can obtain one, by creating a
[mapbox account](https://account.mapbox.com/).

```sh
cd demo_native
cargo run
```

### Web / WASM

```sh
cd demo_web
trunk serve --release
```

### Android

You need to have [Android SDK](https://developer.android.com/) and
[cargo-ndk](https://github.com/bbqsrc/cargo-ndk) installed.

```sh
cd demo_android
make run-on-device
```
