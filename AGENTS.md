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
- Create a decision in the state that is true: `proposed` while the choice is open, `accepted`
  once it is made.
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

- The crates stand in two columns over two shared layers. The Format is `loftspec` over
  `loftspec-store`; Yard Log is `yardlog` over `yardlog-store`. Neither model depends on the
  other model, and neither store on the other store. Beneath both models lies `loft-common`,
  which performs no I/O; beneath both stores lies `loft-ports`, which does.
- `crates/loft-common` owns what the formats share: the reader and writer of the strict YAML
  subset, the `Key` type and the UTC `Time`. It performs no I/O and depends on no other crate of
  the workspace. `loftspec` re-exports each of them where it always had them.
- `crates/loftspec` owns the document model, identity, the canonical byte form, hashing, the
  merge engine, the publisher and the conversion from the v1 export. It performs no I/O and
  depends on no store.
- `crates/loft-ports` owns the machinery both stores share and that knows no document: the
  journaled save over plain paths into a journal directory its caller names, the runner through
  which Loft reaches git, and the clock. It depends on no model and no store.
- `crates/loftspec-store` owns the `DocumentStore` trait, its memory and file implementations and
  the port to git. Every write of the tree, or of a file beside it, goes through one journaled
  save (`journal::save`), whose journal lives in `.loft/save/` beside `spec/` while it runs; a
  command opens the tree through `FileStore::open`, which first finishes or discards an
  interrupted save. Never write a file of the Format any other way. The trait holds only what the
  CLI needs: load, save and roots.
- `crates/yardlog` owns Yard Log's model: its records and keys, the rules a write is held to,
  its canonical YAML and the merge of two copies. It performs no I/O and depends neither on
  `loftspec` nor on a store.
- `crates/yardlog-store` reads and writes each branch's Yard Log under `.yard/<branch>/` through
  `loft-ports`' journaled save, one write at a time under a lock in git's own directory for the
  worktree, and keeps the machine's state in `~/.yard/`. It reaches git only through
  `loft-ports`.
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
- `crates/loft-cli/skill/loft-spec/` is the source of the `loft-spec` agent skill: `SKILL.md`,
  a router whose frontmatter holds only `name` and `description`; `references/`, one workflow
  each; and `agents/`, each host's adaptation (`openai.yaml` for Codex; a frontmatter fragment, a
  route bullet and a host reference under `claude/` for Claude Code). `loft` embeds it, and
  `crates/loft-cli/src/skill/` composes one package per host, which `loft skill install` and
  `loft init` write into a project.
- `plugins/claude/loft/` and `plugins/codex/loft/` are the plugins, each holding its host's
  package under `skills/loft-spec/`; `.claude-plugin/marketplace.json` and
  `.agents/plugins/marketplace.json` list them. The Claude Code plugin also holds the Loft mod,
  one hooks module written by hand that runs Yard Client: `hooks/register.tsx` registers the
  hooks it records events with around Claude Code's own, `loft.ts` runs `loft` and the pure
  `model.ts` reads what it prints, and Yard Client lies in `hooks/yard/`. Its state contract is
  `types/index.d.ts` and its tests are `tests/`, written with Claude Code's test kit. It learns
  of the tree and of Yard Log only from `loft … --format json` (the accepted decision "Yard
  Client is a mod over loft"), and reaches the page server only to post its events. Beside `skills/loft-spec/` it
  holds the `yard` skill, `skills/yard/SKILL.md`, written by hand for Claude Code alone.
  CI runs `claude plugin validate plugins/claude/loft` and `claude plugin test
  plugins/claude/loft` under a pinned Claude Code; run both after a change to the mod, and
  type-check it with TypeScript once a session has loaded the plugin and written its types.
- `crates/loft-cli/yard-page/` is the Yard page, browser-native modules and CSS with no build
  step, which `loft yard serve` embeds and serves; each file is also listed in `ASSETS` in
  `crates/loft-cli/src/yard/serve/assets.rs`. Mermaid is vendored there as
  `vendor/mermaid.min.js.gz` only, at the version its `README.md` pins;
  `scripts/vendor-mermaid.sh` refreshes it. Its tests are in `crates/loft-cli/yard-page-tests/`:
  after `npm ci` there, `npm test` runs them in a DOM emulator and `npm run types` type-checks
  the page, and CI runs both. The browser suite drives the page in a real browser and runs on
  demand only, after `npx playwright install chromium`:
  `cargo nextest run -p loft-cli --run-ignored only -E 'test(/yard_page_browser/)'`. Run it after
  a change to the page or the page server, and before a release.
- Yard Log is kept under `.yard/<branch>/` in a tree, committed with the work unless the
  repository ignores `.yard/`, as this one does. What belongs to the machine, not the branch,
  lies in `~/.yard/`: the page server's `server.json`, its lock and stop request, the trees it
  serves, each session's events, when each session was last heard, the revision each agent last
  read, the sessions that continue others and the control token.
- `npm/loft/` is the launcher of the npm package `@ask-loft/loft`, which runs the prebuilt `loft`
  of the platform it is installed on. A release packs it, and one package per platform, into
  tarballs with `scripts/npm-packages.sh`, and runs `loft` from them with `scripts/npm-smoke.sh`;
  `crates/loft-cli/tests/npm.rs` tests all three.
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

## The agent skill

- Change the skill only in its source, then run `scripts/package-skill.sh` to rewrite the plugin
  packages. Never edit a package under `plugins/` by hand: `crates/loft-cli/tests/plugins.rs`
  fails while a package differs from the composition, or a manifest's version from the version
  of `loft`.
- `SKILL.md` and the references name no host; whatever is particular to a host lies under
  `agents/`. A new source file is also listed in `SOURCE` in `crates/loft-cli/src/skill/mod.rs`.
- Every YAML block of a reference is marked: `path=` blocks form the reference's example tree,
  which passes `loft check --fail-on warning`; a `rule=` block is a wrong example that raises its
  rule and no other warning; a `fragment` is shown and not run. The lint guide describes every
  rule the linter defines. `crates/loft-cli/tests/skill.rs` holds the source to these rules and
  to each reference's opening and contents.
- The `yard` skill is not composed: it lies only in the Claude Code plugin, at
  `plugins/claude/loft/skills/yard/SKILL.md`, and is edited there by hand; the packaging script
  leaves it alone. `crates/loft-cli/tests/yard_skill.rs` holds it to one file of at most 500
  lines, to naming exactly the tools the mod registers, and to each habit its Feature states.

## Software Plant

Loft depends on systems outside it: git, which tells the publisher the current branch, whether
`spec/` has uncommitted changes and whether a merge or rebase is stopped, and the clock, which
dates a publish. Each is a root Component with the boundary `external` in `spec/`, and Loft
reaches it through one port with one real adapter; `loftspec-store` reaches git only through its
repository port. Tests never reach an external system directly and never through a hook in a
shipped binary. The only switches a shipped binary reads for a simulator are its standard and
named environment variables (`SOURCE_DATE_EPOCH`, `LOFT_CLOCK`, `LOFT_TELEMETRY_ENDPOINT`,
`LOFT_UPDATE_ENDPOINT`), and each changes only where its port takes its answers. Tests put a
simulator from the Software Plant in front of the unchanged binary under test:

- **git**: `software_plant::GitScenario` scripts a repository: its prefix, its branch or a
  detached HEAD, its porcelain status, its local configuration (a `config` call sets it for the
  calls after it), a merge or rebase stopped on a conflict with the commit it merges or replays,
  the lock each revision holds and the unmerged index entries, and the version each launcher
  command prints when git runs it through the shell. It also scripts the failures Loft
  anticipates: outside a repository, and a scripted git error. `install` writes the scenario into
  the scratch directory, and `Installed::apply` gives a `Command` a search path whose only `git`
  is the simulator. Any call the simulator does not answer exits 2 naming the call, so Loft
  cannot come to depend on git behaviour nobody modelled.
- **the clock**: the clock port tells the time and waits. `software_plant::Clock` sets
  `SOURCE_DATE_EPOCH`, which `loft publish` reads as its publish time and the command event as
  its hour, fixed or stepping between runs. `Clock::…().passing()` also sets `LOFT_CLOCK=passing`,
  under which a wait returns at once and moves the run's time on by its length, so a run that
  waits minutes ends in milliseconds; every other scenario clears it.
- **the telemetry collector**: `software_plant::Collector` serves the collector's wire contract
  on a loopback port (accepting, failing with a status, silent, or a refused port) and records
  the requests and the events it accepted.
- **GitHub's releases**: `software_plant::ReleaseFeed` answers the latest-release request on a
  loopback port with a release, a draft, a prerelease, an error, a rate limit or silence, and
  counts the requests.

The command event and the update check reach the network only through the network client
(`crates/loft-cli/src/network.rs`), which accepts plain `http` only on the loopback interface,
so `LOFT_TELEMETRY_ENDPOINT` and `LOFT_UPDATE_ENDPOINT` can point a run at a simulator and at
nothing else unencrypted. `.cargo/config.toml` starts every process Cargo runs with both off and
both addresses at a closed loopback port; a test that exercises either switches it on through
`tests/common/telemetry.rs` or `asking` in `tests/update_check.rs`. Run a gate's own `loft`
through `cargo run`, never `target/debug/loft` directly, or set `LOFT_TELEMETRY=internal`.

The rules:

- A new external system is specified first: a root Component with the boundary `external`, a
  port in the crate that uses it, and a Feature of the Software Plant Product that lists the tag
  `plant-simulator` and mentions that Component.
- Every simulator has a contract suite that runs the same scenarios against the simulator and
  against the real system and requires the same answers; the git suite runs real git in scratch
  repositories. A new anticipated failure joins the simulator's scenarios and its contract suite
  in the same change.
- Real git in a test runs through `software_plant::real_git`, which keeps it from the user's
  configuration, from any repository above the temporary directory and, through
  `forget_repository`, from any repository git's environment names: git sets `GIT_DIR` and its
  kin for a hook or `git rebase --exec`, and a test started that way must still find only its
  scratch repository. The command-line harness's `isolate` forgets them too.
- Only a contract suite touches a real external system. Every other test uses the simulator. A
  suite whose real system is a remote service runs its real half only on demand: the half is
  `#[ignore]`, and `--run-ignored only` runs it.
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
- Link each test to the criterion it verifies in `loft.trace.yaml`, by the criterion's key
  (`loft find <label>` prints it), and run `loft reconcile --accept <label>` once the test is
  read against the criterion as it now reads. When a criterion changes, its link turns stale
  (`TR-003`) until the test is reviewed and the link accepted again; when a linked test's tokens
  change, it is reported as changed (`TR-006`), so accept its link in the same commit as the
  change, after `cargo fmt`; a renamed test whose old name still occurs is `TR-009`, so correct
  its link. A test that a part of its file the grammar does not parse may cut short is `TR-010`
  and cannot be accepted: make the file parse there. A criterion that no test can
  verify is linked `unverified` with the reason. Nodes no link names yet are reported as
  `TR-001`; link them as their tests are touched.
- Each crate's integration tests are one binary, `tests/main.rs`, with a module for each file of
  `tests/` and the modules they share; one binary links once, where a binary per file linked
  dozens of times. Declare a new test file there as `mod <file>;`: Cargo no longer finds it by
  itself, and a guard (`crates/loft-cli/tests/test_modules.rs`) fails on a file left out.
- Treat `clippy::too_many_lines` as a hard refactoring gate: never allow, expect, or suppress it.
  When it fires, complete the necessary refactoring within the same turn before continuing or
  handing work off.
- Keep the complexity thresholds in `clippy.toml` enabled. Rust source files have a 500-line
  ceiling. Crossing either gate requires same-turn refactoring, never a raised threshold or budget.
  Guards in `crates/loft-cli/tests/source_limits.rs` hold the ceiling, the thresholds and the
  ban on suppressing `clippy::too_many_lines`.
- Keep every commit green. Do not update golden vectors merely to make a failure disappear.
- Commit each completed step separately; do not combine unrelated steps in one commit.
- Open pull requests against `develop`. `main` carries released, published states only.
- Publish on `main` only, as part of a release (see Releasing).
- Preserve unrelated worktree changes.
- Before every commit of code, run what CI runs, and fix any failure before committing:

  ```sh
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo nextest run --workspace
  cargo run -q -p loft-cli -- fmt --check
  cargo run -q -p loft-cli -- check --fail-on warning
  cargo run -q -p loft-cli -- reconcile --fail-on warning
  ```

  Install the runner with `cargo install cargo-nextest --locked`. nextest runs no doctests, and
  the crates have none; one added later brings `cargo test --workspace --doc` into this list and
  CI with it.
- For docs-only work, run only the checks relevant to the changed documents.
- For a focused loop, use `cargo nextest run -p <crate> <test-name>`.
- Run the workspace suite at most once per commit, and never in a loop: a branch's commits are
  not re-tested one by one, and no two workspace runs share a machine at once, however many agents
  are working. A review reads the diff; it builds and tests nothing and creates no worktree.

## Releasing

A release happens only with the maintainer's approval, and in this order:

0. Run `.github/workflows/build.yml` on `develop` ("Run workflow" on its page, or
   `gh workflow run build.yml --ref develop`) and wait for every platform to build. Ordinary CI
   builds only the host platform, so a change that breaks a cross-build, such as a dependency
   with C code, shows here first.
1. On a branch from `develop`, set the new version in every crate's `Cargo.toml`, in the
   workspace dependencies, in both plugin manifests and in `PLUGIN_VERSION` of the Claude Code
   plugin's `hooks/yard/index.tsx` (`tests/plugins.rs` fails until they agree), and turn `## Unreleased` in `CHANGELOG.md` into `## X.Y.Z (YYYY-MM-DD)`. Merge it
   into `develop`, then `develop` into `main`.
2. On `main`, run `loft publish` over the committed `spec/`, commit `spec/labels.yaml` and
   `loft.lock.json` together as the publish commit, and check that `loft verify` passes. A second
   `loft publish` with nothing changed writes nothing; CI checks both on `main` only, since
   `develop` carries unpublished candidates. Push `main`.
3. On `main`, run `scripts/release-check.sh vX.Y.Z`; it checks `HEAD` while the tag does not
   exist. Then tag `vX.Y.Z` on `main` and push the tag.
4. The tag starts `.github/workflows/release.yml`. It runs the release check and the gate, builds
   `loft` for each supported platform through `build.yml`, packs the npm packages into tarballs, runs `loft` through
   the launcher from them (`scripts/npm-smoke.sh`) and checks them with a dry run, then waits for
   the maintainer to approve: on the run's page, "Review deployments", tick `release`, "Approve and
   deploy". Once approved, the publish job runs the tarballs it downloaded once more,
   `scripts/publish-crates.sh` publishes the crates to crates.io in dependency order,
   `scripts/publish-npm.sh` publishes the platform packages and then `@ask-loft/loft`, and the tag
   gets a GitHub release with the binaries, whose notes are the version's changelog entry. Both
   registries trust the workflow through Trusted Publishing; no registry token is stored anywhere.
5. Merge `main` back into `develop`, so the allocated labels and the release reach it.

When `develop` holds work that is not ready to go out, a release is cut from `main` instead:
branch from `main`, run step 0 on that branch (`gh workflow run build.yml --ref <branch>`), make
the version change of step 1 on it and merge it into `main` alone, then go on from step 2. Step 5
brings the release into `develop`, where `## Unreleased` stays above the new version's entry in
`CHANGELOG.md`.

If a step of the publish job fails, fix its cause and choose "Re-run failed jobs" on the run.
Each step skips what its registry or GitHub already holds, so the run publishes only the rest.

`software-plant` is never published (`publish = false`). CI on `main` runs
`cargo publish --workspace --dry-run`, so a crate that would not package fails before a release.

### Release setup

Set up once, and kept as it is:

- The GitHub environment `release` requires the maintainer's approval, and only tags matching
  `v*` may deploy to it.
- On crates.io, each published crate names the trusted publisher `Ask-Loft/loft`, workflow
  `release.yml`, environment `release`.
- On npmjs.com, each of `@ask-loft/loft` and its platform packages names the same trusted
  publisher, with "Allow npm publish" ticked and "Allow npm dist-tag" left off.

A registry names a trusted publisher only for a package that already exists, and npm requires
2FA for a publish with a token. So a new crate or npm package, such as one for a new platform, is
published by hand the first time, from what the release run built:

- A crate: `cargo publish -p <crate>` from a checkout of the tag, logged in with `cargo login`.
- An npm package: `gh run download <run> -n release`, then
  `scripts/publish-npm.sh release/npm` logged in with `npm login`; npm asks for 2FA for each
  package it publishes and skips those it holds.

Then register its trusted publisher as above, and choose "Re-run failed jobs" on the run, which
completes the release.

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
