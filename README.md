# Loft

The Loft Specification Format and its reference implementation: a specification document model
that one person edits in a git repository and controls with the `loft` command line, with no
server.

- `crates/loftspec` — the format: document model, identity, canonical bytes, hashing, conversion
  from the v1 export
- `crates/loftspec-store` — memory and file stores
- `crates/loft-cli` — the `loft` binary
- `crates/software-plant` — the Software Plant: simulators of the external systems Loft depends
  on, for tests only

Licensed under Apache-2.0 (`LICENSE`, `NOTICE`).

## Status

Pre-release. The format and the command line are specified in this repository's own `spec/`
tree, in the format itself: `loft check` validates and lints it, and `loft render` writes a
readable projection to `docs/spec.md`.

## Software Plant

Loft's commands depend on systems outside Loft: git and the clock. Tests run the real `loft`
binary, unchanged, against simulators from the Software Plant instead of those systems:

- a `git` executable that answers the git calls Loft makes from a scenario the test writes (a
  clean repository on `main`, uncommitted changes, another branch, a detached HEAD, a directory
  outside any repository, a git error) and refuses any other call;
- a clock that sets `SOURCE_DATE_EPOCH`, so a test names the publish time exactly.

```rust
use std::process::Command;
use software_plant::{Clock, GitScenario};

let git = GitScenario::clean_on("main").with(" M", "spec/tags.yaml").install(&scratch);
let mut loft = Command::new(env!("CARGO_BIN_EXE_loft"));
git.apply(&mut loft);
Clock::fixed(1_700_000_000).apply(&mut loft);
```

A contract suite keeps each simulator honest by running the same scenarios against the
simulator and the real system; `cargo nextest run -p loftspec-store git_contract` runs the git
suite against the `git` on your search path. The thinking behind the Software Plant is at
<https://askloft.ai/vision>.

## Build and gate

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo test --workspace --doc
cargo run -q -p loft-cli -- fmt --check
cargo run -q -p loft-cli -- check --fail-on warning
```

Rust 1.97 or newer; `cargo install cargo-nextest --locked` provides the test runner.
