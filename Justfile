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
    # The perf binaries are behind `required-features`, so a plain check skips them.
    cargo check -p demo --features mvt,pmtiles --bins

[group('develop')]
check-wanderers:
    cargo check -p wanderers

[group('develop')]
check: check-lean check-all-features check-demo check-wanderers

[group('develop')]
lints:
    cargo fmt --all --check
    cargo clippy --all-features -- -D warnings
    cargo doc --no-deps

[group('develop')]
typos:
    typos .

[group('develop')]
bench:
    cargo bench -p walkers --features mvt --bench decode_mvt

[group('publish')]
publish *args:
    cargo publish -p walkers {{ args }}
    cargo publish -p walkers_extras {{ args }}

# Bounding box roughly covering Dolnośląskie
DOLNOSLASKIE_BBOX := "14.757385,50.069481,17.341919,51.248163"

# It's the same bbox, but Overpass uses different order (:
DOLNOSLASKIE_OVERPASS_BBOX := "50.069481,14.757385,51.248163,17.341919"

# Wrocław and its surroundings, in Overpass' order.
WROCLAW_OVERPASS_BBOX := "51.030000,16.850000,51.200000,17.200000"

# Overpass answers "406 Not Acceptable" to requests that do not identify themselves.
OVERPASS_USER_AGENT := "walkers (https://github.com/podusowski/walkers)"

# Download hiking trails for Dolnośląskie, Poland from OpenStreetMap using Overpass API and convert to GeoJSON
[group('data')]
overpass-trails-dolnoslaskie:
    curl -G -A '{{ OVERPASS_USER_AGENT }}' https://overpass-api.de/api/interpreter \
        --data-urlencode 'data=[out:json][timeout:120];(relation["route"~"hiking|foot"]["colour"]({{ DOLNOSLASKIE_OVERPASS_BBOX }}););out geom;' \
        -o trails.json
    osmtogeojson trails.json > trails.geojson

# Download water bodies for Wrocław, Poland from OpenStreetMap using Overpass API and convert to GeoJSON
[group('data')]
overpass-water-wroclaw:
    curl -G -A '{{ OVERPASS_USER_AGENT }}' https://overpass-api.de/api/interpreter \
        --data-urlencode 'data=[out:json][timeout:120];(way["natural"="water"]({{ WROCLAW_OVERPASS_BBOX }});relation["natural"="water"]({{ WROCLAW_OVERPASS_BBOX }}););out geom;' \
        -o water.json
    osmtogeojson water.json > water.geojson

# Download mountain peaks for Dolnośląskie, Poland from OpenStreetMap using Overpass API and convert to GeoJSON
[group('data')]
overpass-peaks-dolnoslaskie:
    curl -G -A '{{ OVERPASS_USER_AGENT }}' https://overpass-api.de/api/interpreter \
        --data-urlencode 'data=[out:json][timeout:120];(node["natural"="peak"]["name"]({{ DOLNOSLASKIE_OVERPASS_BBOX }}););out geom;' \
        -o peaks.json
    osmtogeojson peaks.json > peaks.geojson

# Download the latest PMTiles file for Dolnośląskie, Poland from Protomaps.
[group('data')]
protomaps-dolnoslaskie:
    pmtiles extract https://build.protomaps.com/$(date -d 'yesterday' +%Y%m%d).pmtiles --bbox {{ DOLNOSLASKIE_BBOX }} dolnoslaskie.pmtiles
