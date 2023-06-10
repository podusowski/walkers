[![crates.io](https://img.shields.io/crates/v/walkers.svg)](https://crates.io/crates/walkers)

Slippy maps widget for [egui](https://github.com/emilk/egui).

# Limitations

There are couple of limitations when using this library. Some of them will
might probably be lifted at some point. Please raise an issue if you are
particularly affected by some and I will try to prioritize.

* Limited to the OpenStreetMaps, but I want to enable other tile servers and
  protocols (like WMS) as well.
* It uses `reqwests`/`tokio` stack which does not work on WASM.
* Example for Android is missing, but it does work there.


![Screenshot](screenshot.png)
