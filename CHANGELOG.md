# Changelog

All notable changes to this project will be documented in this file.

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
