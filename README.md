# Loft

The Loft Specification Format and its reference implementation: a specification document model
that a person or a small team edits in a git repository and controls with the `loft` command
line, with no server.

- `crates/loftspec` — the format: document model, identity, canonical bytes, hashing, conversion
  from the v1 export
- `crates/loftspec-store` — memory and file stores
- `crates/loft-cli` — the `loft` binary
- `crates/software-plant` — the Software Plant: simulators of the external systems Loft depends
  on, for tests only

Licensed under Apache-2.0 (`LICENSE`, `NOTICE`). Contributions: `CONTRIBUTING.md`. Security
reports: `SECURITY.md`. Changes: `CHANGELOG.md`.

## Status

Released: the latest version and its changes are in `CHANGELOG.md`; 2.0.0 was the first public
release. The format and the command line are specified in this
repository's own `spec/` tree, in the format itself: `loft check` validates and lints it, and
`loft render` writes a readable projection to `docs/spec.md`.

## Working with a specification

You write a specification by talking to a coding agent, not by editing YAML. The agent writes
`spec/` with the `loft-spec` skill and the `loft` command line; you decide what goes in it.

- **Your part.** Say what the product is for and where it ends, answer the agent's questions,
  agree the outline it proposes, choose between the options it lays out, and read what it wrote:
  the diff, or `docs/spec.md` from `loft render`. Then ask it to commit, and to publish. What the
  agent writes in your name is yours to understand before you accept it.
- **The agent's part.** It asks rather than guesses, proposes the outline before writing any
  criterion, writes the nodes, and runs `loft fmt` and `loft check` after every step until both
  pass.
- **Why not by hand.** A change rarely stays in one place: a reworded rule, a split Requirement or
  a new decision touches criteria, edges and citations elsewhere. The agent follows them, and
  `loft check` catches what it misses; keys and references stay `loft`'s to keep, and each
  criterion stays one checkable outcome. A hand edit is still held to the same checks, which
  suits a typo, not a feature.
- **The specification first, then the code.** A change goes into `spec/` as its own commit, then
  into the code, which is checked against the criteria it now has to meet. A gap found while
  coding goes back into the specification, not around it.

## Install

```sh
npm install --save-dev @ask-loft/loft      # a prebuilt binary, in a project that uses npm
cargo install --locked loft-cli            # from crates.io, with the tested dependencies
cargo install --path crates/loft-cli       # from a clone of this repository
```

The npm package carries prebuilt binaries for macOS, Linux and Windows on x64 and arm64, and runs
as `npx loft`; in such a project, register the merge driver with
`npx loft merge-driver --install --command "npx --no-install loft"`. Building from source needs
Rust 1.97 or newer. The binary is `loft`; `loft --version` prints its release, the format and the
hashing algorithm.

The npm package records that npm installed it when it installs. After `cargo install`, or after
unpacking a binary from a GitHub release, `loft record-install cargo` or
`loft record-install release-binary` records the same, so that the update notice names the right
way to update; without it, `loft` infers the way from where it lies.

## Quick start

```sh
cd my-product                   # the product's git repository, or `git init` a new one
loft init                       # spec/, and the loft-spec skill for coding agents
loft merge-driver --install     # once per clone: git merges spec/ by key
git add .gitattributes .claude .agents && git commit -m "Add the Loft specification tools"
```

Then ask your agent, for example: "Use the loft-spec skill to specify a booking system for small
venues" (`/loft-spec` in Claude Code, `$loft-spec` in Codex). It interviews you, agrees an outline
with you, writes `spec/`, runs `loft fmt` and `loft check` until both pass, and commits the draft.
When the specification reads right, ask it to publish, or run:

```sh
loft publish                    # on main: labels for new nodes and loft.lock.json
git add -A && git commit -m "spec: publish"
loft verify                     # in CI: the tree matches the lock
```

`loft init` runs before or after `git init`; its last line says what the merge driver still
needs.

| Command | What it does |
|---|---|
| `loft init [--no-skill]` | Creates an empty document in the current directory, and the agent skill beside it |
| `loft fmt [--check]` | Rewrites `spec/` in canonical layout, mints missing keys, turns references written by name into keys and drops the label bindings of deleted nodes, or reports drift |
| `loft check [--fail-on <severity>]` | Reads, validates and lints `spec/` |
| `loft render [--openspec <dir> [--force]]` | Writes the Markdown projection and the canonical JSON export, or the specification as OpenSpec main specs |
| `loft find <query>` | Prints the key, label, kind, file and name of each node or decision a label, slug, key or name identifies |
| `loft import v1 <export>` | Converts a Loft v1 export into `spec/` |
| `loft import openspec [<dir>]` | Converts the main specs of an OpenSpec project into `spec/` |
| `loft publish` | Allocates labels and writes the lock, on the publish branch |
| `loft verify` | Recomputes the roots and compares them with the lock |
| `loft status [--base <revision>]` | Reports the lock and whether the tree matches it, the repository's branch and uncommitted paths under `spec/`, and the nodes added, changed and removed since HEAD left the revision; it sends no telemetry |
| `loft reconcile [--fail-on <severity>] [--accept <node>] [--accept-stale]` | Checks `loft.trace.yaml`, which links each criterion to the tests that verify it, against the tree and the test files, and records links a reviewer has checked |
| `loft merge-driver` | Merges one file of `spec/` as git's merge driver; `--install` registers it |
| `loft skill install` | Writes the agent skill into the current directory, or brings it up to date |

## Agents

The `loft-spec` skill is how a coding agent writes, changes and reviews a specification with
`loft`, as "Working with a specification" describes: the interview, the edit loop, the format's
files and fields, the writing rules, what each lint finding means, review, and publishing. The agent reads one short router, then only the references a task
needs.

`loft init` writes the skill for Claude Code to `.claude/skills/loft-spec/` and for Codex to
`.agents/skills/loft-spec/`, and enables the Claude Code plugin `loft@ask-loft` in
`.claude/settings.json`, keeping every other entry of that file; `--no-skill` leaves all three
out. `loft skill install` writes it into any
project, and after an upgrade of `loft` brings it to the new version, enabling the plugin the
same way. Commit both directories and `.claude/settings.json` so that everyone working on the
project has the skill and is offered the plugin.

The skill is also a plugin, for an agent that works across projects:

- Claude Code: `/plugin marketplace add Ask-Loft/loft`, then `/plugin install loft@ask-loft`.
  In a project where `loft init` or `loft skill install` ran, the committed `.claude/settings.json`
  already names the marketplace and enables `loft@ask-loft`, so Claude Code offers the plugin
  to whoever trusts the project.
- Codex: `codex plugin marketplace add Ask-Loft/loft`, then `codex plugin add loft@ask-loft`, or
  choose it in `/plugins`.

The plugin teaches the agent; it does not install `loft`, which the skill expects on the path.

In Claude Code the plugin also shows the Loft status pane in a project that holds `spec/`: the
checks of the loop (format, check, `spec/` committed, trace, lock) and the next step, the nodes
the branch added, changed and removed since its base, the criteria it touched that still lack a
reviewed test, and the file the agent is writing. `/loft-status` opens it; `/loft-status
<revision>` compares with another base for the session, and `/loft-status auto` returns to the
`baseBranch` setting (`develop` by default). The pane runs `loft status`, `loft fmt --check`,
`loft check` and `loft reconcile` with telemetry and the update check off; where `loft` is not on
the path, set the plugin's `loftCommand` setting, for example to `pnpm exec loft`.

## Exit codes and JSON

Every command exits 0 on success, 1 on an error, a refusal or a failed check, and 2 on a usage
error. With `--format json` a command prints one JSON document on standard output, described by
its schema under `schemas/` (`schemas/<command>.json`, whose name carries its version);
human-readable text goes to standard error, and the exit code is the same as without it.

Every document carries `schema`, `outcome` and `errors`. `outcome` names the result in words,
including the results that exit 1 without an error: `failed` for `check`, `drift` for
`fmt --check`, `conflicted` for a merge and `mismatch` for `verify`. A command that stops on an
error has the outcome `error`, and its document then holds only those three fields:

```json
{"schema": "loft-fmt/1", "outcome": "error", "errors": [{"kind": "read", "message": "...", "path": "spec/...", "line": 1, "column": 3, "sites": [...]}]}
```

## Publishing and merging

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

## Tracing tests to criteria

`loft.trace.yaml`, beside `spec/`, links each acceptance criterion, and each Constraint or
Quality Attribute whose `verificationFate` is `proved`, to the tests that verify it, by file and
test name, or says why no test can. Each link holds the content hash its node had when a reviewer
last accepted it, so a changed criterion shows up as a stale link, and each test the digest of
its tokens when it was last recorded, so a rewritten test shows up too. `loft reconcile` locates
each test in its file's syntax tree for Rust, TypeScript, JavaScript, Python and Go, and as a
whole word elsewhere, without running it, so it works for any language. It reports nodes no link
names, links not yet reviewed or gone stale, tests changed since they were recorded, recorded
tests it can no longer locate, tests that a part of their file it cannot parse may cut short,
tests it cannot find in their files, and links to nodes that are gone.
`loft reconcile --accept <label>` records a review, and CI runs `loft reconcile --fail-on warning`.
The trace file is not part of the specification: it enters no root and no publish writes it.

## Telemetry and the update check

Telemetry is on by default. At the end of each run of `check`, `find`, `fmt`, `import v1`,
`import openspec`, `init`, `publish`, `reconcile`, `render`, `skill install` and `verify`,
`loft` sends one command event to `https://askloft.ai/api/telemetry`, so that the maintainers
learn which commands are used, how they end and how long they take. Before the first event leaves a machine, `loft` prints
a notice on standard error saying what it sends and how to switch it off. An event looks like
this:

```json
{
  "schema": "loft-telemetry-event/2",
  "eventId": "0b9a3c0e-7f4d-4a43-9a52-3f3f2c4b8d11",
  "version": "2.1.0",
  "os": "macos",
  "arch": "aarch64",
  "installedBy": "npm",
  "time": "2026-10-14T15:00:00Z",
  "installation": "6f1d2c3b-9a8e-4f7d-8c6b-5a4e3d2c1b0a",
  "project": "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d",
  "branch": "9f86d081884c7d65",
  "mode": "agent",
  "agent": "claude-code",
  "command": "check",
  "outcome": "failed",
  "errorKinds": [],
  "duration": "under-1s"
}
```

`schemas/telemetry-event.json` lists every field and every value it may take. The time is the
hour, in UTC; `mode` says whether the run was internal, in CI, by a coding agent, at a terminal or
none of these; `agent` names the agent from the variables its host sets. The three tokens are
random: `installation` is drawn once for the user on the machine, `project` once for a checkout,
in a `.loft/` beside `spec/` that git ignores, and `branch` is the branch name keyed with a secret
kept in that same file, so the same branch in two checkouts gives two different tokens. An event
never carries the specification, the code, a path, a repository address, a name or an error
message. The tokens are pseudonymous rather than anonymous, and the collector sees the network
address each event comes from. Events with tokens are kept for at most 30 days, and aggregates
without them for at most 90 days.

Sending never gets in the way. It starts after the command's output, gives up after 500
milliseconds and prints nothing; a collector that is down, slow or refusing changes no output and
no exit code, and a lost event is never sent again.

`loft` also checks for updates. At the end of the same commands, at most once a day, it asks
GitHub for the latest release and keeps the answer; a later run tells you on standard error,
at most once a day, when a newer release exists and how to install it. The request carries
nothing about you or your project: its User-Agent is `loft`, without even the version, and it
follows no redirect. This check has its own switch and stays on
when telemetry is off.

| To                                   | Run or set                                   |
| ------------------------------------ | -------------------------------------------- |
| switch telemetry off for the user    | `loft telemetry off` (`on` and `status` too) |
| switch telemetry off for one shell   | `LOFT_TELEMETRY=off` (any value but `internal`) |
| switch the update check off          | `loft update-check off`, or `LOFT_UPDATE_CHECK=off` |
| send elsewhere                       | `LOFT_TELEMETRY_ENDPOINT=<url>`              |
| ask elsewhere for releases           | `LOFT_UPDATE_ENDPOINT=<GitHub API base>`     |

`loft telemetry off` deletes the installation token; a variable set to `off` wins over a
recorded `on`, and a recorded `off` wins over any variable. The settings live in `loft/` under
the user's configuration directory (`~/.config` on Linux, `~/Library/Application Support` on
macOS, `%APPDATA%` on Windows); when they cannot be written, `loft` does not check for updates
at all, rather than ask and tell again on every run. Neither address accepts plain `http` except
on the loopback interface. Both requests go through the proxy named by the first of
`ALL_PROXY`, `HTTPS_PROXY` and `HTTP_PROXY` that is set, unless `NO_PROXY` lists the host. `LOFT_TELEMETRY=internal` marks runs by Loft's own maintainers and CI.

## Software Plant

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

## Build and gate

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo run -q -p loft-cli -- fmt --check
cargo run -q -p loft-cli -- check --fail-on warning
cargo run -q -p loft-cli -- verify    # on main, which carries only published states
```

Rust 1.97 or newer; `cargo install cargo-nextest --locked` provides the test runner.

The skill's source lies in `crates/loft-cli/skill/loft-spec/`. After changing it, run
`scripts/package-skill.sh` to rewrite the plugin packages under `plugins/`; a test fails while
they differ.
