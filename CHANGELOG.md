# Changelog

All notable changes to this project will be documented in this file.

## Unreleased

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
