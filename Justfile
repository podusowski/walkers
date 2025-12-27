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
