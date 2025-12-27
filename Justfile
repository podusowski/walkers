help:
    @just --list --unsorted

check-lean:
    cargo check -p walkers

check-all-features:
    cargo check -p walkers --all-features

check-demo:
    cargo check -p demo_native

check: check-lean check-all-features check-demo

publish:
    cargo publish -p walkers
    cargo publish -p walkers_extras
