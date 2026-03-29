help:
    @just --list --unsorted

[group('develop')]
check-lean:
    cargo check -p walkers

[group('develop')]
check-all-features:
    cargo check -p walkers --all-features

[group('develop')]
check-demo:
    cargo check -p demo_native

[group('develop')]
check: check-lean check-all-features check-demo

[group('develop')]
lints:
    cargo fmt --all --check
    cargo clippy --all-features -- -D warnings
    cargo doc --no-deps

[group('develop')]
typos:
    typos .

[group('publish')]
publish:
    cargo publish -p walkers
    cargo publish -p walkers_extras

# Bounding box roughly covering Dolnośląskie
# (south, west, north, east)

BBOX := "50.0,15.9,51.8,17.9"

# Download hiking trails for Dolnośląskie, Poland from OpenStreetMap using Overpass API and convert to GeoJSON
[group('data')]
overpass-trails-dolnoslaskie:
    curl -G https://overpass-api.de/api/interpreter \
        --data-urlencode 'data=[out:json][timeout:120];(relation["route"~"hiking|foot"]["colour"]({{ BBOX }}););out geom;' \
        -o trails.json
    osmtogeojson trails.json > trails.geojson

# Download the latest PMTiles file for Dolnośląskie, Poland from Protomaps.
[group('data')]
protomaps-dolnoslaskie:
    pmtiles extract https://build.protomaps.com/$(date -d 'yesterday' +%Y%m%d).pmtiles --bbox 16.802768,51.036355,17.209205,51.180686 dolnoslaskie.pmtiles
