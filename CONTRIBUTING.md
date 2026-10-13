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
- The `loft-spec` agent skill is changed only in its source, `crates/loft-cli/skill/loft-spec/`;
  then `scripts/package-skill.sh` rewrites the plugin packages under `plugins/`. The source names
  no agent host outside its `agents/` directory, and its YAML examples run as tests.

## Before you open a pull request

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo run -q -p loft-cli -- fmt --check
cargo run -q -p loft-cli -- check --fail-on warning
```

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
