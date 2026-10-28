# Commands

Your coding agent runs these through the `loft-spec` skill; you rarely need to. Every command
takes `--help`.

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
| `loft reconcile [--fail-on <severity>] [--accept <node>] [--accept-stale]` | Checks `loft.trace.yaml` against the tree and the test files, and records links a reviewer has checked ([tracing](tracing.md)) |
| `loft merge-driver` | Merges one file of `spec/` or `.yard/` as git's merge driver; `--install` registers it for both ([publishing and merging](publishing.md)) |
| `loft yard <command>` | Keeps the branch's Yard Log for the Yard adapters: `read`, `write`, `wait`, `withdraw`, `close`, `published`, `answer`, `task`, `continue`, `protocol`, `upgrade`, `serve`, the page server, and each host's hooks ([Yard](yard.md)) |
| `loft skill install [--host <host>] [--no-plugin]` | Writes the agent skill into the current directory, or brings it up to date |
| `loft telemetry`, `loft update-check` | Switch telemetry and the update check on or off ([telemetry](telemetry.md)) |
| `loft record-install <way>` | Records how `loft` was installed, for the update notice; the npm package runs it itself ([telemetry](telemetry.md)) |

## The agent skill

`loft init` writes the `loft-spec` skill for Claude Code to `.claude/skills/loft-spec/` and for
Codex to `.agents/skills/loft-spec/`, and enables the Claude Code plugin `loft@ask-loft` in
`.claude/settings.json`, keeping every other entry of that file; `--no-skill` leaves all three
out. After an upgrade of `loft`, `loft skill install` brings the skill to the new version.
`--host codex` or `--host claude-code` installs for one host only, and `--no-plugin` leaves
plugin settings alone. Commit both directories and `.claude/settings.json`, so that everyone
working on the project has the skill and is offered the plugin. The `yard` skill ships with each
host's plugin, not through `loft skill install`.

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
