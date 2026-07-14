FROM rust:1.97-alpine AS builder

RUN apk add --no-cache musl-dev
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --locked --release --bin velux2mqtt

FROM alpine:3.21

RUN addgroup -S velux2mqtt && adduser -S -G velux2mqtt velux2mqtt
COPY --from=builder /build/target/release/velux2mqtt /usr/local/bin/velux2mqtt
USER velux2mqtt
ENTRYPOINT ["/usr/local/bin/velux2mqtt"]
