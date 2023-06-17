# Changelog

All notable changes to this project will be documented in this file.

## Unreleased

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
