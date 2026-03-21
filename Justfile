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

trails-dolnoslaskie:
    curl -G https://overpass-api.de/api/interpreter \
        --data-urlencode 'data=[out:json][timeout:60];(relation["route"="hiking"]({{ BBOX }}););out geom;' \
        -o trails.json
    osmtogeojson trails.json > trails.geojson

    # [out:json][timeout:120];area["name"="województwo dolnośląskie"]->.a;relation(area.a)["type"="route"]["route"="hiking"];out tags;way(r);out geom;
