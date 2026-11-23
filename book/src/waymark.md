# Vector tiles

[OpenFreeMap](https://openfreemap.org)'s vector tiles, drawn with Waymark, Walkers' own style.

<iframe class="demo" src="demos/index.html#waymark" loading="lazy"></iframe>

Vector tiles carry the map's geometry and names rather than a picture of them, so it is the style
that decides how they look. A `StyledSources` draws a `Style` from sources it names, here
OpenFreeMap's tiles fetched by an `HttpSource`. Waymark draws from a source called
`openmaptiles`, after the [OpenMapTiles](https://openmaptiles.org/schema/) schema those tiles
follow, and comes in the colors of a `Palette`, light or dark.

```rust,noplayground
{{#include ../../demos/src/waymark.rs:new}}
```

`StyledSources` keeps the tiles it has fetched, so, as `HttpTiles` for raster tiles, it is
created once and added to the map as a layer in every frame. A map can have more than one source,
so there is a link to each one's attribution.

```rust,noplayground
{{#include ../../demos/src/waymark.rs:ui}}
```

Vector tiles are drawn with wgpu, so `walkers::install_renderer` has to be called once on startup,
with eframe's `wgpu_render_state`:

```rust,noplayground
{{#include ../../demos/src/main.rs:renderer}}
```

[Full source](https://github.com/podusowski/walkers/blob/main/demos/src/waymark.rs)
