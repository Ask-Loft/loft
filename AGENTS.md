# Loft Specification Format agent guide

This repository is the open home of the Loft Specification Format and its reference
implementation: the `loftspec` library, the `loftspec-store` stores and the `loft` command line.
It is Apache-2.0 and public from release 2.0.0; nothing in it is confidential.

## Normative contract

The contract is this repository's own specification in `spec/`, written in the Format's text
form. When code and `spec/` disagree, `spec/` wins unless the task explicitly changes the
contract.

The normative state is the one `loft.lock.json` pins: the state root of `spec/` at the last
publish on `main`, which `loft verify` recomputes and CI checks there. Edits to `spec/` on
`develop` and on feature branches are candidates that the next publish makes normative.

- Change the specification in `spec/` in the same pull request as the code, and before the code
  in the history: a commit of the specification, then the code that meets it.
- Write new nodes without keys and run `loft fmt`, which mints them. Labels (`RQ-12`, `AC-40`)
  are allocated only by `loft publish` on `main`; a node added since the last publish has a key
  and no label.
- Create a decision `proposed` and accept it in a later change, never accepted at birth.
- Keep `loft fmt --check` and `loft check --fail-on warning` clean; CI runs both.
- `loft render` writes `docs/spec.md` and `docs/spec.json` on demand. They are never committed
  and never normative.
- Plans and authoring records are kept outside this repository.

## Agreement with v1

Loft v1 implements the same structural and language rules (`SH-*`, `LQ-*`), EARS forms and label
rules, and this repository leads them (the accepted decision "v2 leads the behaviour it shares
with v1"). Change shared behaviour through `spec/`. An intended difference from v1 is an entry of
`testdata/v1/divergences.json` in the same pull request, naming its rule, condition, direction and
the criteria or decisions in `decidedBy`. `tests/v1_parity.rs` compares loftspec with
`v1-lint.json` and `v1-ears.json` beside each public v1 export under `testdata/v1/`. Those files
are written only by a v1 implementation; never edit or regenerate them to make a failure
disappear. When v1's output changes, the files are rewritten from v1 after `spec/` adopts the
change, or v1 reverts it.

## Repository layout

- `crates/loftspec` owns the document model, identity, the canonical byte form, hashing, the
  merge engine, the publisher and the conversion from the v1 export. It performs no I/O and
  depends on no store.
- `crates/loftspec-store` owns the `DocumentStore` trait, its memory and file implementations and
  the port to git. Every write of the tree, or of a file beside it, goes through one journaled
  save (`journal::save`), whose journal lives in `.loft/save/` beside `spec/` while it runs; a
  command opens the tree through `FileStore::open`, which first finishes or discards an
  interrupted save. Never write a file of the Format any other way. The trait holds only what the
  CLI needs: load, save and roots.
- `crates/loft-cli` is the `loft` binary and owns nothing the library could own.
- `crates/software-plant` is the Software Plant: simulators of the external systems Loft depends
  on, for tests only. It is never published and never a dependency of a published crate.
- None of these ever depends on a crate of the private Loft server. That boundary is the licence
  boundary.
- Identifiers are typed domain IDs — `ProductId`, `FeatureGroupId`, `FeatureId`,
  `RequirementId`, `CriterionId`, `ComponentId`, `ConstraintId`, `QualityAttributeId`,
  `SectionId`, `DecisionId`, `OptionId`, `DriverId`, `ConsequenceId`, `TagId`, `CategoryId` — and
  references by name are typed (`TagName`, `CategoryName`); never a plain string or one shared
  key type. Every typed ID wraps the element's key and serialises as it, so the files, views and
  digests are unchanged by the types. A kind change keeps the key through an explicit
  conversion. Prose (names, bodies, statements) stays text.
- `spec/` is the specification. `schemas/` holds the JSON schema of each command's output.
  `testdata/` holds synthetic fixtures, the golden vectors under `testdata/golden/` and the v1
  exports the converter is tested on.

## Commands and their output

Every command exits 0 on success, 1 on an error, a refusal or a failed check, and 2 on a usage
error. With `--format json` it prints one JSON document on standard output, and human-readable
text goes to standard error only.

- Every document carries `schema`, `outcome` and `errors`. `outcome` names the result in words,
  including each result that exits 1 without an error (`failed`, `drift`, `conflicted`,
  `mismatch`). A command that stops on an error has the outcome `error`, and its document then
  holds only those three fields. `crates/loft-cli/src/json.rs` builds both shapes (`result`,
  `failure`); build every document through it.
- Each command has one schema, `schemas/<command>.json`, that names its version and states the
  result shape and the error shape with `oneOf`. A change to a command's output changes its
  schema in the same commit; once released, that change is a new version of the schema.
- A new command needs its Feature in `spec/`, with the tag `reads-tree` if it reads the tree and
  `writes-tree` if it writes a file of the Format; an entry in
  `crates/loft-cli/tests/common/commands.rs`; and its schema. The suites in `reads_tree.rs`,
  `writes_tree.rs` and `schemas.rs` then hold it to every shared rule, and fail until each of
  those is in place.

## Software Plant

Loft depends on systems outside it: git, which tells the publisher the current branch, whether
`spec/` has uncommitted changes and whether a merge or rebase is stopped, and the clock, which
dates a publish. Each is a root Component with the boundary `external` in `spec/`, and Loft
reaches it through one port with one real adapter; `loftspec-store` reaches git only through its
repository port. Tests never reach an external system directly and never through a hook in a
shipped binary. They put a simulator from the Software Plant in front of the unchanged binary
under test:

- **git**: `software_plant::GitScenario` scripts a repository: its prefix, its branch or a
  detached HEAD, its porcelain status, its local configuration (a `config` call sets it for the
  calls after it), a merge or rebase stopped on a conflict with the commit it merges or replays,
  the lock each revision holds and the unmerged index entries, and the version each launcher
  command prints when git runs it through the shell. It also scripts the failures Loft
  anticipates: outside a repository, and a scripted git error. `install` writes the scenario into
  the scratch directory, and `Installed::apply` gives a `Command` a search path whose only `git`
  is the simulator. Any call the simulator does not answer exits 2 naming the call, so Loft
  cannot come to depend on git behaviour nobody modelled.
- **the clock**: `software_plant::Clock` sets `SOURCE_DATE_EPOCH`, which `loft publish` reads as
  its publish time, fixed or stepping between runs.

The rules:

- A new external system is specified first: a root Component with the boundary `external`, a
  port in the crate that uses it, and a Feature of the Software Plant Product that lists the tag
  `plant-simulator` and mentions that Component.
- Every simulator has a contract suite that runs the same scenarios against the simulator and
  against the real system and requires the same answers; the git suite runs real git in scratch
  repositories. A new anticipated failure joins the simulator's scenarios and its contract suite
  in the same change.
- Only a contract suite touches a real external system. Every other test uses the simulator.
- A system that calls Loft, rather than one Loft calls, gets no simulator: Loft never calls it,
  so there is nothing for a simulator to answer. Its contract suite runs Loft under the real
  system instead. git's merge machinery calls `loft merge-driver`, so
  `crates/loft-cli/tests/merge_contract.rs` runs the parallel-work scenarios under a real
  `git merge` with the driver registered, and every other merge test calls the engine or the
  driver directly.
- `software-plant` holds simulators only. Fixtures and conformance harnesses stay in the crates
  they test.

In the command-line tests, `Scratch::loft` runs `loft` with no `git` on its search path and a
fixed clock, as every command but a publish needs; a test that needs a repository or another
time passes its own `GitScenario` and `Clock` to `Scratch::loft_with`.
`crates/loft-cli/tests/publish.rs` is the worked example.

The thinking behind the Software Plant is at <https://askloft.ai/vision>.

## Workflow

- Work test-first: add a focused failing test, implement the smallest correct behavior, then
  refactor. Check each key behaviour negatively too: break it and see a test fail.
- Test names say what they prove in words and cite no label: a publish can withdraw a label and
  give its node a new one, and a guard (`crates/loft-cli/tests/citations.rs`) fails on a label
  in a test name.
- Each crate's integration tests are one binary, `tests/main.rs`, with a module for each file of
  `tests/` and the modules they share; one binary links once, where a binary per file linked
  dozens of times. Declare a new test file there as `mod <file>;`: Cargo no longer finds it by
  itself, and a guard (`crates/loft-cli/tests/test_modules.rs`) fails on a file left out.
- Treat `clippy::too_many_lines` as a hard refactoring gate: never allow, expect, or suppress it.
  When it fires, complete the necessary refactoring within the same turn before continuing or
  handing work off.
- Keep the complexity thresholds in `clippy.toml` enabled. Rust source files have a 500-line
  ceiling. Crossing either gate requires same-turn refactoring, never a raised threshold or budget.
- Keep every commit green. Do not update golden vectors merely to make a failure disappear.
- Commit each completed step separately; do not combine unrelated steps in one commit.
- Open pull requests against `develop`. `main` carries released, published states only.
- Publish on `main` only, and only with the maintainer's approval: merge `develop` into `main`,
  run `loft publish` there over a committed `spec/`, commit `spec/labels.yaml` and
  `loft.lock.json` together as the publish commit, check that `loft verify` passes, then merge
  `main` back into `develop` so the allocated labels reach it. A second `loft publish` with
  nothing changed writes nothing; CI checks both on `main` only, since `develop` carries
  unpublished candidates.
- Preserve unrelated worktree changes.
- Before every commit of code, run what CI runs, and fix any failure before committing:

  ```sh
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo nextest run --workspace
  cargo test --workspace --doc
  cargo run -q -p loft-cli -- fmt --check
  cargo run -q -p loft-cli -- check --fail-on warning
  ```

  nextest runs no doctests, so `cargo test --doc` is what keeps the `compile_fail` boundaries
  checked; install the runner with `cargo install cargo-nextest --locked`.
- For docs-only work, run only the checks relevant to the changed documents.
- For a focused loop, use `cargo nextest run -p <crate> <test-name>`.

## Public hygiene

Nothing confidential enters this repository, in any commit: no export of the private server's
documents, no plans or strategy documents, no authoring records, no secrets, no `.loft/` or
`.env` files, no customer data. The git history is public with the repository. Commit messages,
code comments and test names are public too: they never name the private server's documents,
states or repositories. Describe such sources generically, as "a large converted document" or "a
local v1 export".

## Definition of done

Behavior is complete only when `spec/` states it and `loft check --fail-on warning` is clean,
the same conformance scenario passes against the memory and file stores, the golden vectors under
`testdata/golden/` still match, every command's JSON matches its schema, and formatting and
Clippy are clean.
