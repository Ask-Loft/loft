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

`loft publish`, run on the publish branch (`main` unless `spec/loftspec.yaml` names another) over
a committed tree, allocates labels to new nodes in `spec/labels.yaml` and writes
`loft.lock.json`, which pins the specification's state, content and binding roots. `loft verify`
recomputes those roots and compares them with the lock; CI runs it on `main`. A release merges
`develop` into `main`, publishes there, commits the labels and the lock together, and merges
`main` back into `develop`.

`loft merge-driver --install`, run once in a clone, registers Loft as git's merge driver for
`spec/`: it adds `spec/**/*.yaml merge=loft` to `.gitattributes` and sets `merge.loft.driver` in
the clone's configuration. git then merges each file of the tree by key. Where two branches
change one field apart, the file gains a conflict record in place of conflict markers, git
reports the path as conflicted, and `loft check` reports the record until it is removed.

git runs the driver through the shell, from the top of the working tree, with its own search
path, which a dev dependency is not on. Give the command that starts `loft` there with
`--command`: `loft merge-driver --install --command "npx --no-install loft"`, `"pnpm exec loft"`,
`"uv run loft"`, or a path such as `node_modules/.bin/loft`. The install runs
`<command> --version` as git would and refuses when it fails. The setting lives in the clone's
`.git/config`, so a project runs the install in its setup step, such as npm's `prepare` script.
If the command cannot run during a merge, the driver merges the file as text with conflict
markers and leaves it conflicted; make the command run, then `git checkout -m <file>` merges it
again.

Two clones that publish apart can give one label to two nodes. When one pulls the other, by
`git pull` (a merge) or `git pull --rebase`, the merge driver keeps the disputed bindings out of
`spec/labels.yaml` in one conflict record and git stops. `loft publish`, run there on the
publish branch, settles them: the pulled side keeps its labels, the other side's colliding
labels are withdrawn and their nodes get fresh ones, citations of a withdrawn label are
rewritten in every element that mentions its node, and the lock is written anew. It then prints
the git command that finishes, `git add -A && git commit --no-edit` after a merge or
`git add -A && git rebase --continue` after a rebase; Loft itself never changes the repository.

## Software Plant

Loft's commands depend on systems outside Loft: git and the clock. Tests run the real `loft`
binary, unchanged, against simulators from the Software Plant instead of those systems:

- a `git` executable that answers the git calls Loft makes from a scenario the test writes (a
  clean repository on `main`, uncommitted changes, another branch, a detached HEAD, a local
  configuration, a merge or rebase stopped on a conflict, a directory outside any repository, a
  git error) and refuses any other call;
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
suite against the `git` on your search path. git also calls Loft, as the merge driver;
`cargo nextest run -p loft-cli merge_contract` runs the parallel-work scenarios under a real
`git merge`, with no simulator in between. The thinking behind the Software Plant is at
<https://askloft.ai/vision>.

## Build and gate

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo test --workspace --doc
cargo run -q -p loft-cli -- fmt --check
cargo run -q -p loft-cli -- check --fail-on warning
cargo run -q -p loft-cli -- verify    # on main, which carries only published states
```

Rust 1.97 or newer; `cargo install cargo-nextest --locked` provides the test runner.
