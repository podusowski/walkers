# Changelog

All notable changes to this project will be documented in this file.

## 0.51.0

* `vector_tiles` Cargo feature is now split into `mvt` and `pmtiles`.
* Basic support for vector styling. See `Style` struct for details. You can use it with `Tiles`
  implementations that support vector tiles, e.g. `PmTiles::with_style()`.
* Added `OpenFreeMap` source. Note that OpenFreeMap serves vector tiles and those remain highly
  experimental.
* `LocalTiles` is now deprecated. For local maps, use `PmTiles` with local `.pmtiles` file.

## 0.50.0

* Another round of improvements to experimental vector tile rendering.
* If raster image cannot be decoded, Walkers will try to interpret it as a vector tile. This means
  that raster and mvt are supported in both PmTiles and HTTP tile sources.
* `Texture::from_color_image` is no longer public.
* `Texture` is renamed to `Tile`.
* `TextureWithUv` is renamed to `TilePiece`.

## 0.49.0

* More improvements to experimental vector tile rendering.
* `Map::show` now receives the `egui::Response` just like plugins do.
* KML support in `walkers_extras`. See `kml_viewer` for details.

## 0.48.0

* `rstar` based `GroupedPlaces` implementation, which makes it considerably faster.
* `walkers_extras` is now a separate crate.
* Further work on experimental vector tile rendering.

## 0.47.0

* `egui` updated to 0.33.
* MSRV updated to 1.88.

## 0.46.0

* New `LocalTiles` implementation for loading tiles from a local directory.
* New, highly experimental vector tile rendering using `.pmtiles`. It has very poor performance and
  at this point it should be considered more of a tech demo than a usable feature.
* Removed `Image` plugin.
* Fixed zooming on high refresh rates.

## 0.45.0

* `Map::show()` now passes `&MapMemory` to the closure.

## 0.44.0

* New `Map::show()` function. It takes a closure that can draw custom content on the map. It is
  similar to plugins, but more idiomatic to `egui` which might help to avoid some of the workarounds
  needed in the previous approach.
* `Map::drag_gesture()` is replaced by `Map::drag_pan_buttons()`, which allows configuring which
  mouse buttons can be used for dragging.
* New `MapMemory::animating()` function, which returns whether the map is currently animating.

## 0.43.0

* `egui` updated to 0.32.
* MSRV set to 1.85.

## 0.42.0

* Fixed bug preventing map from entering detached state when `Memory::set_zoom` is called every
  frame.
* New `Map::pull_to_my_position_threshold` function for setting the threshold of the feature
  introduced in 0.40.0. It defaults to `0.0`, meaning that the map will not be pulled at all.

## 0.41.0

* `Plugin::run()` has a new parameter `map_memory`, which allows plugins to access it after it is
  modified by the map.
* `GroupedPlaces` now takes an instance of `Group` (e.g. `LabeledSymbolGroup`), which allows
  groups to be customized.
* `LabeledSymbolGroup` now has the `style` field, which allows changing how the group looks.
* `LabeledSymbol::symbol` is now optional and can have a different types. See `Symbol` enum
  for possible values.
* `LabeledSymbolStyle` has new fields `label_corner_radius` and `symbol_size`.

## 0.40.0

* When map is dragged or zoomed while not being in a detached state, it will get pulled back to
  `my_position`, unless offset reaches a certain threshold.

## 0.39.0

Please note that places plugin family are now being intensively refactored and might change pretty
radically in the next releases.

* `Place` is now renamed to `LabeledSymbol`, which implements a new trait called... `Place`.
* `Style` is now renamed to `LabeledSymbolStyle`.
* `Images` plugin is replaced by `Places`, which simply accepts both `LabeledSymbol` and `Image`.
* New `GroupedPlaces` plugin, which groups multiple instances of `LabeledSymbol` when they are too
  close to each other.
* New `Map::panning` function which can be used to disable panning.

## 0.38.0

* Make slow-down during inertial moves exponential rather than linear.
* `MaxParallelDownloads`, `max_parallel_downloads`'s type is now public, allowing to actually set it.

## 0.37.0

* Support for multiple layers.
* `MapMemory` now implements `serde::Serialize` and `serde::Deserialize` when the `serde` feature is
  enabled.
* New setting `HttpOptions::max_parallel_downloads`.

## 0.36.0

* `screen_to_position` is no longer a public function. Use `Projector::unproject` to obtain
  geographical coordinates from screen coordinates relative to the map viewport.
* New `HttpTiles::stats()` function.
* Fixed download attempts of invalid tiles in certain cases.

## 0.35.0

* Fixed zooming on touchscreens so that it follows the center of the touch points.
* `Projector::unproject` is now symmetric to `Projector::project`. Previously its origin was the
  screen center and often needed adjusting.

## 0.34.0

* `egui` updated to 0.31.0

## 0.33.0

* Do not try to download tiles with invalid coordinates.
* `Position` is now a type alias for `geo_types::Point`. Previous `from_lat_lon` and `from_lon_lat`
  methods are now standalone functions called `lat_lon` and `lon_lat`.

## 0.32.0

* `egui` updated to 0.30.0.

## 0.31.0

* Tile download optimized by no longer queueing older requests.
* Interpolation of higher zoom levels is now improved, trying many different levels instead of just
  one.

## 0.30.0

* Add option for changing the zoom speed
* Add options for double click to zoom in and out
* Add option to zoom without holding ctrl on native and web
* Fixed zoom not following cursor if the map wasn't moved.
* Fixed spurious "Error from IO runtime" at shutdown.

## 0.29.0

* Do not set a user agent in wasm build by default.
* Fixed not taking into account the mouse pointer position when scrolling.

## 0.28.0

* Implement panning with touchpad and/or scroll-wheel.

## 0.27.0

* Fixed crash when zoom is maxed far out.
* Add `Project::scale_pixel_per_meter()` to provide the local meter-to-pixel scale.
* Map can be zoomed in farther than what is supported by the tile provider. The max. zoom level
  for which tile images are available can be configured via `TileSource::max_zoom()`.

## 0.26.0

* `HttpTiles` will now attempt to use already downloaded tiles with a lower zoom level as
  placeholders.
* `Tiles::at()` now returns a new `TextureWithUv` instead of `Texture`. This change is relevant
  only for `Tiles` implementers and provides the ability to use part of the texture as a tile.
* Zoom is now represented as `f64` instead of `f32` which makes it consistent with other types.
* `Plugin::run()` has a new signature. Refer to `demo/src/plugins.rs` for usage.

## 0.25.0

* `egui` updated to 0.29.1.

## 0.24.0

* `egui` updated to 0.28.

## 0.23.0

* New functions in `MapMemory` for getting and setting the zoom level: `zoom` and `set_zoom`.
* In-memory cache is now limited to 256 tiles. Previously it grew indefinitely.
* `TilesManager` trait is now called `Tiles` and `Tiles` struct is now called `HttpTiles`.

## 0.22.0

* `egui` updated to 0.27.

## 0.21.0

* New `HttpOptions::user_agent` field. Note that in case of providers like OSM, it is highly
  advised to set it accordingly to the application's name.
  https://operations.osmfoundation.org/policies/tiles/

## 0.20.0

* Fix weird quirks while dragging by small amounts.
* Plugins: Fixed problem of handle clicks after update to egui 0.26
* Map can be zoomed to decimal zoom levels with gestures or scrolling.

## 0.19.0

* `egui` updated to 0.26.

## 0.18.0

* Tiles are now downloaded in parallel.
* `Plugin::draw()` is now called `Plugin::run()` and no longer has `gesture_handled` argument,
  in preference to `egui::Response::changed()`.
* New `Projector::unproject()` function converts screen coordinates to a geographical position.
* New `Projector::new()` allows `Projector` to be used outside plugins.

## 0.17.0

* `egui` updated to 0.25.

## 0.16.0

* `mod providers` is now called `mod sources`, to resemble `trait TileSource`.
* HTTP cache can be now enabled on native platforms (in WASM, is it handled by the browser).
* `TileManager` trait and demonstration of locally generated tiles.
* Zoom and drag gestures can now be disabled.
* Add `gesture_handled` to `Plugin::draw()` to let plugins know if the gesture was handled by the map.

## 0.15.0

* `egui` updated to 0.24. This change requires Rust 1.72 or greater.

## 0.14.0

* Fixed occasional panic when changing tile providers.
* Fixed grabbing mouse events from outside the widget.
* `Position` can be now converted into `geo_types::Point`.

## 0.13.0

* `Position` is no longer a typedef of `geo_types::Point`. There is also a new, more explicit
  way of constructing it - `from_lat_lon`, and `from_lon_lat`.
* If center position is detached, zooming using mouse wheel will now keep location under pointer
  fixed.
* In `Images` plugin, `scale` and `angle` functions are now part of `Image`.
* Allow structs implementing `Provider` to use larger tile sizes.
* Add optional logos to `Attribution` struct.
* Add `Mapbox` provider.
* `Plugin::draw` now has a `Response` parameter, allowing plugins to be interactive.

## 0.12.0

* `egui` updated to 0.23.

## 0.11.0

* `Zoom` type is no longer public, while `InvalidZoom` becomes public.
* `MapMemory::center_mode` is no longer public,
* New `MapMemory::follow_my_position` function.
* Fix occasional disappearing of the map when dragging rapidly.

## 0.10.1

* Brought back the ability to center the map at exact position (`MapMemory::center_at`) after
  making some types private.

## 0.10.0

* `Images` plugin, for putting images at geographical location.
* `Projector` and `MapMemory` are now `Clone`.
* `MapMemory::zoom` is no longer `pub`. Use `MapMemory::zoom_in/out` instead.
* `MapMemory::center_mode::detached()` is no longer `pub`. Use `MapMemory::detached()` instead.
* Fixed weird drag behavior in higher zoom levels.

## 0.9.0

* Tile sources are now defined via `TileSource` trait, instead of `Fn`.
* New `Plugin` trait and `Map::with_plugin` function, which replaces `Map::with_drawer`.
* Example plugin `extras::Places`, which draws markers on the map.

## 0.8.0

* Previous example was split into `demo` library, and `demo_*` integrations.
* Support for WASM.

## 0.7.0

### Breaking

* New `Center` variant - `Inertia`. It means that the map is moving due to inertia and
  soon will stop with `Center` switching to `Exact`. To keep things easy, `Center` now
  has new method `detached`, will returns a position the map is currently at, or `None`
  if it just follows `my_position`.
* `openstreetmap` is now in `walkers::providers` module.
* `osm` example is now called `myapp` and it shows a small windows with an orthophotomap
  layer from <https://geoportal.gov.pl>.

### Added

* New method: `Map::with_drawer`, which provides a simpler API for drawing custom stuff.

## 0.6.0

### Breaking

* `MapCenterMode` is now called `Center`.
* `Zoom` can no longer be dereferenced to `u8`. To obtain a previous value, use `Zoom::round()`.
* Also, `Zoom` is no longer `PartialEq`, nor `Eq`.

### Added

* Zooming using CTRL + mouse wheel or pinch gesture.

### Fixed

* Fixed panic when dragging out of the map's boundaries.

## 0.5.0

### Breaking

* `Tiles::new()` has now two parameters. First, called `source`, being a function transforming
  `TileId` into an URL of a tile provider. This means that it's now possible to specify other
  servers. `openstreeetmap` is a builtin function returning OSM's URL.
* `MapMemory` no longer has `osm` member. Instead, `Map::new`'s `tiles` parameter is now `Option`.

### Fixed

* Optimized how GUI and IO thread talk to each other.
* Handle tile download errors (HTTP statuses, garbage content, etc.) more gracefully. Application
  will remain alive (no `unwrap` or `expect`), but downloads of the failed tiles won't be repeated.

## 0.4.0

### Breaking

* `TileId::position_on_world_bitmap` is now called `project` and it returns `Pixels` .
* `PositionExt::project_with_zoom` is now called `project` and it returns `Pixels` .
* `Tiles::new()` has now a single parameter - `egui_ctx` . It can be obtained from egui's
  `CreationContext` (see the example).

### Fixed

* Fixed calculation of the amount by which map should be scrolled during the screen drag.
* Map did not get repainted when a tile got downloaded in the background, unless some mouse
  movement was made.

## 0.3.0

### Added

* Tokio runtime is now managed by the `MapMemory` so constructing it by hand is no longer necessary.
* Example updated with drawing custom shapes on the map.
