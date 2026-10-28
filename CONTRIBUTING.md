# Contributing

Thank you for helping. `AGENTS.md` is the full guide to this repository, for people and for coding
agents alike; this page is the short version.

## The specification comes first

The contract is this repository's own specification in `spec/`, written in the Loft
Specification Format. A change of behaviour changes `spec/` in the same pull request as the
code: write the new Requirements and criteria without keys, run `loft fmt` to mint them, and keep
`loft check --fail-on warning` clean. A decision is added in the state that is true, `proposed` or
`accepted`. When code and `spec/` disagree, `spec/` wins unless the change is to the contract.

Labels (`RQ-12`, `AC-40`) are allocated only by `loft publish` on `main`, at release. Nodes you
add have keys and no labels until then.

## Code

- Work test-first: a focused failing test, the smallest change that passes it, then refactor.
- Test names say what they prove in words and never cite a label; a guard fails on one.
- Functions stay within the thresholds in `clippy.toml` and files within 500 lines. Lints are
  never allowed or suppressed: refactor instead.
- Tests never reach git or the clock directly. They put a simulator from the Software Plant
  (`crates/software-plant`) in front of the unchanged `loft` binary; only a contract suite runs the
  real system.
- Behaviour shared with Loft v1 is led from here. An intended difference is an entry in
  `testdata/v1/divergences.json`.
- A change to a command's output changes its schema under `schemas/` in the same commit.
- The agent skills are changed only in their sources, `crates/loft-cli/skill/loft-spec/` and
  `crates/loft-cli/skill/yard/`; then `scripts/package-skill.sh` rewrites the plugin packages under
  `plugins/`, and a test fails while they differ. The sources name no agent host outside their
  host adaptations, and the `loft-spec` skill's YAML examples run as tests.
- After a change to the Claude Code plugin's mod, run `claude plugin validate plugins/claude/loft`
  and `claude plugin test plugins/claude/loft`. After a change to the Yard page, run its tests in
  `crates/loft-cli/yard-page-tests/` (`npm ci`, then `npm test` and `npm run types`).

## The Software Plant

Loft's commands depend on systems outside Loft: git, the clock, the telemetry collector and
GitHub's releases. Tests run the real `loft` binary, unchanged, against simulators from the
Software Plant instead of those systems:

- a `git` executable that answers the git calls Loft makes from a scenario the test writes (a
  clean repository on `main`, uncommitted changes, another branch, a detached HEAD, a local
  configuration, a merge or rebase stopped on a conflict, a directory outside any repository, a
  git error) and refuses any other call;
- a clock that sets `SOURCE_DATE_EPOCH`, so a test names the publish time exactly;
- a collector on a loopback port that accepts, rejects, fails, stays silent or refuses
  connections as the collector's wire contract says, and records what it received;
- a release feed on a loopback port that answers GitHub's latest-release request with a
  release, a draft, a prerelease, an error or a rate limit, or stays silent.

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
`git merge`, with no simulator in between. The collector and release-feed suites,
`collector_contract` and `release_feed_contract` in `loft-cli`, run their real halves only on
demand (`--run-ignored only`): against the collector named by `LOFT_COLLECTOR_CONTRACT_URL`, and
against GitHub. Every process Cargo starts in this repository, the tests included, begins with
telemetry and the update check off (`.cargo/config.toml`); a test that exercises either switches
it on and points it at a simulator. The thinking behind the Software Plant is at
<https://askloft.ai/vision>.

Two suites run on demand only. The Yard page's browser suite needs
`npx playwright install chromium` first:
`cargo nextest run -p loft-cli --run-ignored only -E 'test(/yard_page_browser/)'`. The native
Codex contract needs Codex 0.164.0 and Python 3, and uses a loopback model fixture with no API
account: `cargo nextest run -p loft-cli --run-ignored only -E 'test(real_codex_hooks)'`.

## Working on Yard

On Unix, `scripts/yard-dogfood.sh` starts Claude Code or Codex against this checkout's `loft`,
with Python 3, `ps` and `lsof` installed:

```sh
scripts/yard-dogfood.sh claude
scripts/yard-dogfood.sh claude resume <session-id>
scripts/yard-dogfood.sh codex
scripts/yard-dogfood.sh codex resume <session-id>
```

Each launch builds the release binary, restarts the shared page server on port 7412 even when
the version number is unchanged, and confirms it is serving before starting the agent. You can
invoke the script by its path from another directory; the agent starts in that directory.
Concurrent launches serialize their builds and restarts. Other running sessions reconnect
without restarting their agents; refresh open Yard pages to load files from a same-version
rebuild. Registered trees and Yard state are kept. A build failure leaves the current server
running, and a restart failure stops the launch with an error. Detached server output is kept in
`~/.yard/dogfood-server.log`. Restart an agent session to load changed adapter hooks.

## Before you open a pull request

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo run -q -p loft-cli -- fmt --check
cargo run -q -p loft-cli -- check --fail-on warning
cargo run -q -p loft-cli -- reconcile --fail-on warning
```

Building needs Rust 1.97 or newer, and `cargo install cargo-nextest --locked` provides the test
runner. On `main`, which carries only published states, CI also runs `loft verify`.

Every commit stays green. Open pull requests against `develop`; `main` carries released, published
states only.

## Releases

Releases are cut by the maintainer: a tag `vX.Y.Z` on `main` runs the release workflow, which
publishes the crates to crates.io and the prebuilt binaries to npm after the maintainer approves
it. `AGENTS.md` lists the steps.

## Public by default

Everything here is public, the history included. Never commit secrets, private documents or
customer data, and keep commit messages, comments and test names free of them.

## Licence

Contributions are made under the Apache License 2.0, as section 5 of `LICENSE` states.
