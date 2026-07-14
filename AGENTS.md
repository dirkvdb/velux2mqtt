# Repository Guidelines

## Development Environment

Use the repository's devenv shell:

```text
devenv shell
```

The shell intentionally includes both Rust and Node.js. Rust is the implementation target; Node.js remains the
behavioral reference during the port.

## Quality Gates

Run these commands before considering a Rust change complete:

```text
just check
cargo build --locked --release
```

`just check` runs rustfmt, clippy with warnings denied, and the nextest suite. Use `just test-cargo` when nextest is
not available. Container changes must also pass `just docker-build` and run `velux2mqtt --help` in the image.

## Protocol Rules

- Treat a TLS read as an arbitrary byte chunk, never as a complete SLIP or KLF frame.
- Validate protocol ID, declared length, checksum, payload lengths, and fixed-width strings before decoding.
- Preserve unknown command IDs and enum values.
- Keep KLF-native position semantics inside `klf200`: 0% is open and 100% is closed.
- Never log passwords or raw authentication payloads.
- Add golden vectors and malformed/truncated cases for every new request or response codec.
