set dotenv-load := true


default: check

build:
    cargo build --all-targets

release:
    cargo build --release

test:
    cargo nextest run --all-features

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

clippy:
    cargo clippy --all-targets --all-features -- -D warnings

check: fmt-check clippy test

run *args:
    cargo run --bin velux2mqtt -- {{args}}

mqtt-probe *args:
    cargo run --example mqtt-probe -- {{args}}

docker-build:
    docker build --tag velux2mqtt:local .
