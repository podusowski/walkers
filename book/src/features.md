# Your own data

This map shows a walk through Wrocław's old town, with a drone circling over it. Both come from
the app's own data, and the map's style draws them like the rest of the map.

<iframe class="demo" src="demos/index.html#features" loading="lazy"></iframe>

## The data

Your app's own points, routes and areas are a `Source`. A `MemorySource` keeps them in memory.
It holds a list of `FeatureLayer`s. Each one has a name and a list of `Feature`s. A feature is a
shape in longitude and latitude, with properties the style can read.

```rust,noplayground
{{#include ../../demos/src/features.rs:features}}
```

## Drawing it

Style layers draw your data the same way they draw tiles. Each layer names the source, and one of
its layers as its `source-layer`. Here, a fill draws the island, and a line draws the route.
Symbol layers label the places and the drone. The drone also gets an icon from the style.

```rust,noplayground
{{#include ../../demos/src/features.rs:style}}
```

## Adding it to the map

You add these layers to Waymark's style, between its own layers:

- the walk goes under the map's labels, so it does not hide street names
- its labels go on top, so they win where labels overlap

Then `with_source` adds each `MemorySource` to the `StyledSources`. Use the same name the layers
use.

```rust,noplayground
{{#include ../../demos/src/features.rs:new}}
```

## Changing it

A `MemorySource` and its clones share the same features. Keep one clone in your app, and call
`set` on it when your data changes. The map draws the new features from the next frame.

Walkers styles your features again in every frame. This works well for tens or hundreds of
features, even if they change every second.

```rust,noplayground
{{#include ../../demos/src/features.rs:ui}}
```

[Full source](https://github.com/podusowski/walkers/blob/main/demos/src/features.rs)
