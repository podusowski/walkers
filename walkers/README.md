# Walkers, a map widget for Rust

![made in Europe](https://img.shields.io/badge/made_in-Europe-blue)
[![crates.io](https://img.shields.io/crates/v/walkers.svg)](https://crates.io/crates/walkers)
[![docs.rs](https://img.shields.io/docsrs/walkers/latest)](https://docs.rs/walkers/latest/)

Walkers is a slippy maps widget for [egui](https://github.com/emilk/egui), 
similar to very popular [Leaflet](https://leafletjs.com/), but written in Rust.
It compiles to native applications as well as WASM. See the **[online demo here](https://podusowski.github.io/walkers/)**.

![Screenshot](https://raw.githubusercontent.com/podusowski/walkers/main/screenshot.png)

It supports [OpenStreetMap](https://www.openstreetmap.org), [mapbox](https://www.mapbox.com/), 
and compatible tile servers as well as off-line tiles using the [PMTiles](https://protomaps.com/pmtiles/) format.

Before deploying your application, please get yourself familiar with the
[OpenStreetMap usage policy](https://operations.osmfoundation.org/policies/tiles/), 
and consider donating to the [OpenStreetMap Foundation](https://supporting.openstreetmap.org/).

## Features

- Fetching tiles over HTTP from XYZ tile servers.
- Reading tiles from local `.pmtiles` files.
- Raster tiles rendering.
- Vector tiles (MVT) rendering with styling similar to [MapLibre style](https://maplibre.org/maplibre-style-spec/).
- Experimental local `.geojson` files support.

## Quick start

Walkers has three main objects. `Tiles` downloads images from a tile map provider
such as OpenStreetMap and stores them in a cache, `MapMemory` keeps track of
the widget's state and `Map` is the widget itself.

```rust
use walkers::{HttpTiles, Map, MapMemory, MercatorProjection, Position, sources::OpenStreetMap, lon_lat};
use egui::{Context, CentralPanel};
use eframe::{App, Frame};

struct MyApp {
    tiles: HttpTiles<MercatorProjection>,
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
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut Frame) {
        ui.add(
            Map::new(MercatorProjection, &mut self.map_memory, lon_lat(17.03664, 51.09916))
                .with_layer(&mut self.tiles, 1.0)
        );
    }
}
```

You can see a more complete example [here](https://github.com/podusowski/walkers/blob/main/demo/src/lib.rs).

## Running the demo

### Native

To run demo application locally, use a default cargo run target.

```sh
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

## Offline maps

To obtain offline maps in `.pmtiles` format, you can fetch Dolnośląskie region extract from
[Protomaps](https://docs.protomaps.com/guide/getting-started) using:

```sh
just protomaps-dolnoslaskie
```

You can also use [Overpass API](https://overpass-api.de/) to fetch hiking trails in GeoJSON format:

```sh
just overpass-trails-dolnoslaskie
```

## Mapbox support

To enable **mapbox** layers, you need to define `MAPBOX_ACCESS_TOKEN` environment
variable before building. You can get one by creating a
[mapbox account](https://account.mapbox.com/).
