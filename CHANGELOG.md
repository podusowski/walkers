# Changelog

All notable changes to this project will be documented in this file.

## Unreleased

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
