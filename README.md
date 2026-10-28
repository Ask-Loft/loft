# Loft

The Loft Specification Format and its reference implementation: a specification document model
that a person or a small team edits in a git repository and controls with the `loft` command
line. It needs no server; a document can opt in to publishing through one, as a connected
document.

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
| `loft init [--no-skill \| --no-plugin]` | Creates an empty document in the current directory, and the agent skill beside it; `--no-plugin` leaves `.claude/settings.json` alone |
| `loft fmt [--check]` | Rewrites `spec/` in canonical layout, mints missing keys, turns references written by name into keys and drops the label bindings of deleted nodes, or reports drift |
| `loft check [--fail-on <severity>] [--baseline <revision>]` | Reads, validates and lints `spec/`; with `--baseline`, marks each finding introduced or preexisting against the tree where HEAD left the revision, and gates on the introduced ones |
| `loft render [--openspec <dir> [--force]]` | Writes the Markdown projection and the canonical JSON export, or the specification as OpenSpec main specs |
| `loft find <query>` | Prints the key, label, kind, file and name of each node or decision a label, slug, key or name identifies |
| `loft import v1 <export>` | Converts a Loft v1 export into `spec/` |
| `loft import openspec [<dir>]` | Converts the main specs of an OpenSpec project into `spec/` |
| `loft publish` | Allocates labels and writes the lock, on the publish branch |
| `loft verify` | Recomputes the roots and compares them with the lock |
| `loft status [--base <revision>]` | Reports the lock and whether the tree matches it, the repository's branch and uncommitted paths under `spec/`, and the nodes added, changed and removed since HEAD left the revision; it sends no telemetry |
| `loft review [--base <revision>] [--node <node>] [--max-chars <n>] [--page <k>]` | Gathers the context each assertion under review is read against, as a JSON packet paged by whole units, and judges nothing; with `--base`, only what the branch added, changed, moved or removed |
| `loft terms` | Lists the names the specification may be using for one concept, and the subjects of each Product's statements, for the term list to decide |
| `loft reconcile [--fail-on <severity>] [--accept <node>] [--accept-stale]` | Checks `loft.trace.yaml`, which links each criterion to the tests that verify it, against the tree and the test files, and records links a reviewer has checked |
| `loft merge-driver` | Merges one file of `spec/` or `.yard/` as git's merge driver; `--install` registers it for both |
| `loft yard <command>` | Keeps the branch's Yard Log for Yard Client: `read`, `write`, `wait`, `withdraw`, `close`, `published`, `answer`, `continue`, `protocol`, `upgrade`, and `serve`, the page server |
| `loft skill install [--no-plugin]` | Writes the agent skill into the current directory, or brings it up to date; `--no-plugin` leaves `.claude/settings.json` alone |

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

Use `loft skill install --host codex` or `--host claude-code` to install only that host; an
omitted `--host`, or `--host all`, installs both. A Codex-only install leaves Claude Code's files
and settings untouched. `--no-plugin` leaves plugin settings untouched when installing for
Claude Code.

In this repository, `.agents/skills/loft-spec` is a relative link to the source under
`crates/loft-cli/skill/loft-spec/`, so Codex discovers the same router and references that we
edit and ship. Installation preserves that link and reports it as unchanged. Broken or
unexpected skill links are refused before any selected skill or plugin settings are written.

The skill is also a plugin, for an agent that works across projects:

- Claude Code: `/plugin marketplace add Ask-Loft/loft`, then `/plugin install loft@ask-loft`.
  In a project where `loft init` or `loft skill install` ran, the committed `.claude/settings.json`
  already names the marketplace and enables `loft@ask-loft`, so Claude Code offers the plugin
  to whoever trusts the project.
- Codex: `codex plugin marketplace add Ask-Loft/loft`, then `codex plugin add loft@ask-loft`, or
  choose it in `/plugins`.

The plugin teaches the agent; it does not install `loft`, which the skill expects on the path.

In Claude Code the plugin runs `loft` itself, with telemetry and the update check off; where
`loft` is not on the path, set the plugin's `loftCommand` setting, for example to
`pnpm exec loft`.

### Yard Client

Yard's core keeps the log, submissions, reports and page independently of the agent host.
Adapters translate a host's native tools and events into Yard's JSON commands and events;
Claude Code and Codex have native adapters. Sessions are identified by host as well as session
and subagent, so different hosts can share a branch without sharing an agent's state.

In Claude Code the plugin keeps each branch's Yard Log: what the agent planned, asked,
decided and did on the branch, kept as YAML under `.yard/<branch>/` and committed with the work.
The `yard` skill teaches the agent to open it as work on a branch begins, to give each step a
stage, to send the questions of one decision point together and wait for the answers, and to
record what each step produced and Loft's reports of the specification's work. The questions it
asks with `AskUserQuestion`, the plans it presents with `ExitPlanMode` and its tasks reach Yard
Log by themselves.

The Yard page, at `http://127.0.0.1:7411/`, shows every Yard Log of the trees on the machine,
live: the stages and blocks, a Gantt chart of the session, Loft's reports, the tokens each stage
took, and what waits on you. Answer and approve there or in Claude Code's own dialog: whichever
comes first counts, and the other closes. `loft yard serve` serves the page on the loopback
interface only; the plugin starts it and keeps one running for the machine. Set the plugin's
`yardPort` setting to serve another port.

The page reports each adapter's capabilities. Unreported usage is shown as unavailable, and a
subtotal missing contributing counters is marked partial. The Claude adapter supports Yard
protocols 1 and 2; protocol 2 carries explicit host identity and adapter capabilities.

In local Codex sessions, the plugin's native command hooks run `loft yard codex hook` and a
background `loft yard codex watch`. Use a build of `loft` that provides those commands, install
the Codex plugin as above, then review and trust its hooks in `/hooks`; installing a skill alone
does not activate them. Codex may require another review after a hook definition changes.
Keep hooks enabled in Codex's configuration (`features.hooks = true`). This integration is for
local Codex workflows and is not available with cloud orchestration.
The plugin uses Codex's native manifest: Codex 0.164.0 ignores lifecycle hooks in a manifest
that declares the Agent Plugins schema.

The Codex `yard` skill uses the six existing Yard CLI operations and sends questions and plans
to the Yard page. Page approval records your decision; Codex's own mode and permission controls
still govern execution. Successful `update_plan` calls mirror tasks into the caller's active
stage when Codex offers that tool; otherwise the skill maintains tasks through explicit Yard
writes. Native dialogs are not synchronized with the page. Usage counters are unavailable, and
tool activity with ambiguous subagent identity is omitted. No transcripts are parsed.

Codex owns its background worker, which sends a heartbeat every five seconds, recovers the page
server and retains at most 1,000 queued events. Each worker lasts at most 24 hours; later hooked
activity restarts one that ended. Adapter bookkeeping lives in Codex's plugin data directory,
with a fallback under `~/.yard/codex/`. The on-demand native contract requires Codex 0.164.0 and
Python 3 and uses a loopback model fixture, with no API account:
`cargo nextest run -p loft-cli --run-ignored only -E 'test(real_codex_hooks)'`.

Tasks can be assigned explicitly to another agent while staying in their original stage. Set a
task's `assignee` to `{ "host": "codex", "session": "SESSION", "agent": "AGENT", "agentType": "TYPE" }`
(without the agent fields for a main agent). The stage owner or current assignee can reassign or
release unfinished work; a worker can claim an unassigned task for itself. Read the latest
revision before writing. The page identifies the assignee separately from the stage owner and
keeps separate execution attempts and concurrent task bars without counting overlapping time twice.

After assignment, Claude native task operations bind with `metadata.yardTask: "TASK-ID"` on
every update. Codex native plan steps bind with `[yard:TASK-ID] Local wording`. These references
update the existing task in its original stage, without requiring the worker to own a stage.
Unbound native tasks retain their normal mirroring behavior. An observed end or interruption
returns active assignments to open, retaining the assignee; it never marks unfinished work done.
Activity and reported tokens use the assigned task's stage where unambiguous. Active assignments
in different stages remain unassigned to a stage unless an event explicitly identifies a current
assignment; reported counters still contribute to session totals. Restart agent sessions to load
updated adapter hooks, and refresh the Yard page after rebuilding the server.

The Yard skill is composed from a shared workflow and each host's adaptation under
`crates/loft-cli/skill/yard/`. `scripts/package-skill.sh` updates both plugin packages alongside
the specification skill. Standalone `loft skill install` installs the specification skill;
the Yard skill ships with its adapter's plugin.

A repository that keeps its plans off git ignores `.yard/`. Run `loft merge-driver --install` so
that a merge joins two copies of a Yard Log record by record. What belongs to the machine, such
as the page server's address and each session's events, is kept in `~/.yard/`.

For development on Unix, use this checkout's `scripts/yard-dogfood.sh` with Python 3, `ps` and
`lsof` installed:

```sh
scripts/yard-dogfood.sh claude
scripts/yard-dogfood.sh claude resume <session-id>
scripts/yard-dogfood.sh codex
scripts/yard-dogfood.sh codex resume <session-id>
```

Each launch builds the release binary, restarts the shared Yard server on port 7412 even when
the version number is unchanged, and confirms it is serving before starting the agent. You can
invoke the script by its path from another directory; the agent starts in that directory.
Concurrent launches serialize their builds and restarts. Other running Claude and Codex
sessions reconnect without restarting their agents; refresh open Yard pages to load files from
a same-version rebuild. Registered trees and Yard state are kept. A build failure leaves the
current server running, and a restart failure stops the launch with an error. Detached server
output is kept in `~/.yard/dogfood-server.log`.

## Exit codes and JSON

Every command exits 0 on success, 1 on an error, a refusal or a failed check, and 2 on a usage
error. With `--format json` a command prints one JSON document on standard output, described by
its schema under `schemas/` (`schemas/<command>.json`, whose name carries its version);
human-readable text goes to standard error, and the exit code is the same as without it.

Every document carries `schema`, `outcome` and `errors`. `outcome` names the result in words,
including the results that exit 1 without an error: `failed` for `check`, `drift` for
`fmt --check`, `conflicted` for a merge and `mismatch` for `verify`. A command that stops on an
error has the outcome `error`, and its document then holds only those three fields. A read
error of the tree names the kind of the reader's problem in `problem`, such as `syntax`:

```json
{"schema": "loft-fmt/2", "outcome": "error", "errors": [{"kind": "read", "problem": "syntax", "message": "...", "path": "spec/...", "line": 1, "column": 3, "sites": [...]}]}
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
each test in its file's syntax tree for Rust, TypeScript, JavaScript, Python, Go and Java, and
as a whole word elsewhere, without running it, so it works for any language. It reports nodes no
link names, links not yet reviewed or gone stale, tests changed since they were recorded,
recorded tests it can no longer locate, tests that a part of their file it cannot parse may cut
short, tests it cannot find in their files, and links to nodes that are gone.
`loft reconcile --accept <label>` records a review, and CI runs `loft reconcile --fail-on warning`.
The trace file is not part of the specification: it enters no root and no publish writes it.

## Telemetry and the update check

Telemetry is on by default. At the end of each run of `check`, `find`, `fmt`, `import v1`,
`import openspec`, `init`, `publish`, `reconcile`, `render`, `skill install`, `terms` and `verify`,
`loft` sends one command event to `https://askloft.ai/api/telemetry`, so that the maintainers
learn which commands are used, how they end and how long they take. Before the first event leaves a machine, `loft` prints
a notice on standard error saying what it sends and how to switch it off. An event looks like
this:

```json
{
  "schema": "loft-telemetry-event/2",
  "eventId": "0b9a3c0e-7f4d-4a43-9a52-3f3f2c4b8d11",
  "version": "2.2.1",
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
`ALL_PROXY`, `HTTPS_PROXY` and `HTTP_PROXY` that is set, unless `NO_PROXY` lists the host. `LOFT_TELEMETRY=internal` marks runs by Loft's own maintainers.

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
