# Loft

The Loft Specification Format and its reference implementation: a specification document model
that one person edits in a git repository and controls with the `loft` command line, with no
server.

- `crates/loftspec` — the format: document model, identity, canonical bytes, hashing, conversion
  from the v1 export
- `crates/loftspec-store` — memory and file stores
- `crates/loft-cli` — the `loft` binary

Licensed under Apache-2.0 (`LICENSE`, `NOTICE`).

## Status

Pre-release. The format and the command line are being specified before they are built.
`docs/loft.lock.json` pins the Loft document that holds that specification during the bootstrap,
and `docs/loft-authored-state.json` and `docs/loft-authored-state.md` are its exports.

## Build and gate

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo test --workspace --doc
```

Rust 1.97 or newer; `cargo install cargo-nextest --locked` provides the test runner.
