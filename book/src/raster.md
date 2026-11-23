# Raster tiles

[OpenStreetMap](https://www.openstreetmap.org)'s raster tiles, the shortest way to a map.

<iframe class="demo" src="demos/index.html#raster" loading="lazy"></iframe>

Raster tiles are pictures, drawn as they come. `HttpTiles` fetches them from a tile server and
keeps them, `MapMemory` keeps the map's position and zoom between frames, and `Map` is the widget
itself, built anew in every frame.

```rust,noplayground
{{#include ../../demos/src/raster.rs}}
```

Tile servers ask to be credited, so the closure passed to `show` puts a link to the attribution
on top of the map. Before shipping an app using OpenStreetMap's tiles, read its
[tile usage policy](https://operations.osmfoundation.org/policies/tiles/).
