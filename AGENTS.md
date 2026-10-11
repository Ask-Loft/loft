# Loft Specification Format agent guide

This repository is the open home of the Loft Specification Format and its reference
implementation: the `loftspec` library, the `loftspec-store` stores and the `loft` command line.
It is Apache-2.0 and becomes public at release 0.1.0; it is private until then only so the early
history can be tidy, never because anything in it is confidential.

## Normative contract

During the bootstrap, the contract is the Loft document D-11 "Loft Specification Format" as pinned
by `docs/loft.lock.json`. `docs/loft-authored-state.json` is that immutable Authored State as
`get_authored_state` returns it, and `docs/loft-authored-state.md` is its `render_markdown`
projection; both must match the lock. When code and the pinned state disagree, the pinned state
wins unless the task explicitly changes the contract. Authoring runs from `../loft/` with the
`loft` skill and the connected production Loft MCP session; this repository keeps only the lock
and the exports. Plans and authoring records are kept outside this repository.

While a D-11 work branch is ahead of the lock, `docs/loft.work.json` names the exact Branch
Checkpoint that implementation targets. It is a **candidate**, never the contract: validate it
against the lock and the checkpoint record with the `loft` skill's
`validate_repository_work_target.py`, read the checkpoint's content through the connected MCP
session, and cite its Requirement and criterion labels in tests as candidate labels. When that
exact checkpoint's merge is Published, one commit refreshes the lock and both exports and removes
`docs/loft.work.json`.

The bootstrap ends at the self-hosting milestone: the document is imported into `spec/`,
`loft publish` writes a v2 lock whose `derivedFrom` names the last v1 state, and from then on
`spec/` is the source.

## Repository layout

- `crates/loftspec` owns the document model, identity, the canonical byte form, hashing and the
  conversion from the v1 export. It performs no I/O and depends on no store.
- `crates/loftspec-store` owns the `DocumentStore` trait and its memory and file implementations.
- `crates/loft-cli` is the `loft` binary and owns nothing the library could own.
- None of these ever depends on `loft-domain`, `loft-store` or any other crate of the private
  server. That boundary is the licence boundary.
- Identifiers are typed domain IDs — `ProductId`, `FeatureGroupId`, `FeatureId`,
  `RequirementId`, `CriterionId`, `ComponentId`, `ConstraintId`, `QualityAttributeId`,
  `SectionId`, `DecisionId`, `OptionId`, `DriverId`, `ConsequenceId`, `TagId`, `CategoryId` — and
  references by name are typed (`TagName`, `CategoryName`); never a plain string or one shared
  key type. Every typed ID wraps the element's key and serialises as it, so the files, views and
  digests are unchanged by the types. A kind change keeps the key through an explicit
  conversion. Prose (names, bodies, statements) stays text.
- `docs/` holds the lock, the exports and, while one is active, the work target. `testdata/` holds
  synthetic fixtures and golden vectors only.

## Workflow

- Work test-first: add a focused failing test, implement the smallest correct behavior, then refactor.
- Treat `clippy::too_many_lines` as a hard refactoring gate: never allow, expect, or suppress it.
  When it fires, complete the necessary refactoring within the same turn before continuing or
  handing work off.
- Keep the complexity thresholds in `clippy.toml` enabled. Rust source files have a 500-line
  ceiling. Crossing either gate requires same-turn refactoring, never a raised threshold or budget.
- Keep every commit green. Do not update golden vectors merely to make a failure disappear.
- Commit each completed plan stage separately; do not combine stages in one commit.
- For a contract change during the bootstrap, use the `loft` workflow from `../loft/`: author on a
  D-11 work branch, validate and lint, checkpoint, Publish only with explicit approval, then refresh
  the lock and both exports from the authenticated `get_authored_state` and `render_markdown` reads
  of that exact immutable state and verify the bundle before implementation relies on it. Never
  treat an unpublished Draft as normative.
- Preserve unrelated worktree changes.
- Before handing work off, run `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace` and
  `cargo test --workspace --doc`. nextest runs no doctests, so the last command is what keeps the
  `compile_fail` boundaries checked; install the runner with `cargo install cargo-nextest --locked`.
- For docs-only work, run only the checks relevant to the changed documents.
- For a focused loop, use `cargo nextest run -p <crate> <test-name>`.

## Public hygiene

Nothing confidential enters this repository, in any commit: no export of the private server's
document D-1, no plans or strategy documents, no authoring records, no secrets, no `.loft/` or
`.env` files, no customer data. The git history becomes public with the repository.

## Definition of done

Behavior is complete only when the same conformance scenario passes against the memory and file
stores, the golden vectors under `testdata/golden/` still match, formatting and Clippy are clean,
and the pinned Authored State reflects deliberate contract changes.
