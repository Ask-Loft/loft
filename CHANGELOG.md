# Changelog

Releases of the `loft` command line and the `loftspec` and `loftspec-store` libraries. The
Format and the hashing algorithm carry their own versions, which `loft --version` prints.

## Unreleased

- `loft status [--base <revision>]` reports where the tree stands: the lock's publish sequence,
  publication time and state root, and the first root of the tree that differs from it; the
  branch, the uncommitted paths under `spec/` and a stopped merge or rebase; and with `--base`
  the nodes added, changed and removed since HEAD left the revision, each with its key, label,
  kind, file, line, column and name. A node changed when its own content changed, so an edit is
  not a change of each of its ancestors, and a move is no change. It writes nothing, exits 0
  whatever it finds, and sends no command event, since an agent host may run it every few
  seconds. Its output follows the schema `loft-status/1`.
- The Claude Code plugin shows the Loft status pane in a project that holds `spec/`: the checks
  of the loop with the next step, the nodes the branch changed since its base, the criteria it
  touched that still lack a reviewed test, the file the agent is writing, and a status-line
  entry. `/loft-status [<revision>|auto]` opens it; the settings `baseBranch` and `loftCommand`
  choose the base and the command that runs `loft`. It needs a `loft` with `loft status`.
- `loft init` and `loft skill install` also enable the Claude Code plugin `loft@ask-loft` in the
  project's `.claude/settings.json`: the marketplace `ask-loft`, from GitHub's `Ask-Loft/loft`, and
  the plugin on, so Claude Code offers the plugin to whoever trusts the project. Every other
  entry of the file is kept in its order, an entry it already sets is kept (an opt-out
  included), and a file that is not a JSON object is left alone with the entries to add by hand.
  `--no-skill` leaves the file out too.
- Both plugin marketplaces are now named `ask-loft`: install with `/plugin install loft@ask-loft`
  in Claude Code and `codex plugin add loft@ask-loft` in Codex.

## 2.1.0 (2026-10-15)

- `loft import openspec [<dir>]` converts the main specs of an OpenSpec project, read from
  `openspec/` by default, into `spec/`, and writes `openspec-sidecar.json` beside it with the
  source of each node, including the requirement and scenario names. Each Requirement keeps its
  OpenSpec name and its metadata lines, such as `**ID**: AUTH-1`, as `{label, value}` entries in
  `ext.openspec`, and each Feature its capability id and any archive placeholder Purpose, so no
  tool data is left in a body. A Purpose that opens with a `TBD` or `TODO` marker is a
  placeholder kept there too. A statement whose `MUST` was written as `shall` keeps `MUST` in
  `ext.openspec.modal`, and one stored with a lower-case `shall` keeps `shall`. Headings above
  the first level-2 heading, and inside a requirement's child that is not a scenario, become
  nested Sections. The Product is named after
  the directory that holds the OpenSpec directory, or by `--product`. Active and archived changes,
  `config.yaml`'s `context`, and every place a project departs from OpenSpec's grammar are
  reported as findings (`OS-001` to `OS-015`; `OS-009` is retired) with their source file and line. OpenSpec states
  obligations in scenario bodies, which become criterion bodies, so the command also reports how
  many `V2-007` warnings `loft check` will give and the `spec/lint.yaml` entry that lowers them to
  info; it writes no lint configuration. In text, it ends with `next:` lines: run
  `loft skill install` where the skill is missing, register the merge driver as `loft init`
  suggests, and adopt the import with the skill's workflow. Its output follows the schema
  `loft-import-openspec/1`; the JSON document carries no suggestion.
- The `loft-spec` skill has a workflow for adopting an OpenSpec project: the import findings
  that ask for work, the `V2-007` entry for the adoption period, Components, criteria rewritten
  as checkable sentences, placeholder Purposes, Decisions and Constraints lifted from the
  project, and an OpenSpec projection kept with `loft render --openspec`.
- `loft render --openspec <dir>` writes the specification as the main specs of an OpenSpec
  directory, with `loft-render-map.json` beside them naming the node each requirement and scenario
  comes from, and prints each render finding (`OR-001` to `OR-012`). It refuses a main specs
  directory that already holds files other than ones whose names begin with `.`; `--force`
  replaces the main specs in it once every file is written, prints each one it removes, never
  removes below an entry beginning with `.`, and changes nothing else. It refuses a symbolic
  link on the way to a file it writes, two Features that would be written to one path (letter
  case aside), and any main specs directory that would lie under `spec/`. Without `--markdown`
  or `--json` it writes neither of the other projections. Its output follows the schema
  `loft-render/2`, which adds `removed` and `findings`.
- The command event is now `loft-telemetry-event/2`: its `command` may be `import openspec`.
- **Breaking (library):** a conversion's sidecar is a `loftspec::convert::SidecarFile`, its
  name with its bytes. `Imported::sidecar` is one, `FileStore::import` takes one, and the v1
  sidecar's name moves from `loftspec_store::fs::SIDECAR` to `loftspec::convert::v1::SIDECAR`.
- `loftspec_store::openspec::read_project` reads an OpenSpec directory into the files the
  OpenSpec converter takes, and `loftspec::convert::openspec::layout` names its parts.
- **Format:** every node record may hold `ext`, the data a tool keeps on a node that the Format
  does not define, under the tool's own namespace: text, integers, `true` and `false`, lists and
  mappings. No lint rule reads it, no projection but the JSON export shows it, and a merge
  treats it as one value. It is not part of a node's content hash, so it never makes a trace
  link stale; the state root holds it through a new extensions root (domain
  `loftspec/extensions/1`), and a document in which no node carries `ext` keeps the state root
  it had. A reader from 2.0.4 or earlier refuses a tree that uses `ext`.
- **Breaking (library):** `Details` and `Section` gain `ext`, `Roots` gains `extensions`, and
  `Domain` gains `Extensions`; `loftspec::ext` holds the value model and
  `loftspec::names::Namespace` the namespace.
- `loftspec::render::openspec::openspec` renders a document's OpenSpec projection: the main
  specs of one OpenSpec directory per Product, each Feature at its capability path, a render map
  from each rendered requirement and scenario to its key and label, and findings (`OR-001` to
  `OR-012`) for what OpenSpec 1.14's validator would reject or warn about, read as it reads a
  spec, for the content the projection leaves out, for content a second import would read
  differently and for `ext.openspec` values it cannot read. It returns a `RenderError`, which
  refuses two Features written to one path. An imported project renders back to the specs it
  was read from. `EarsForm::render_with_modal` writes a statement with another modal word.
- `loftspec::lint::count_body_rule` counts the findings of one lexical rule that reads bodies
  only, such as `V2-007`, without running the rest of the linter.

- `V2-002` names the EARS form a statement stored as text follows. A sentence such as "The
  writer SHALL write keys." follows the ubiquitous form by its words, so `LQ-007` accepts it, as
  v1 does, but the reader keeps slots only for the template's exact wording and stores it as
  text. Its `V2-002` finding now carries the form's name as its term and says so; a sentence
  that follows no form keeps the finding without a term.
- **Fixed:** the JSON output of `loft import v1` named each file of the tree with `spec/` twice,
  as `spec/spec/categories.yaml`; it now names each file at its path from the root.
- **Fixed:** `loft render` refused a projection under `spec/` only when its path was relative
  and in the tree's letter case; an absolute path, or on a file system that ignores case another
  letter case, now is refused too.
- **Breaking (library):** `loftspec_store::fs::write_outputs` returns a `WrittenOutputs`, the
  paths it wrote and the paths it left unchanged; `read_project` fails with a `StoreError`, which
  gains `NotText`; `write_projection` takes the projection by value. The OpenSpec conversion and
  render findings are both a `loftspec::report::InfoFinding` of their own rule.

## 2.0.4 (2026-10-14)

- `loft reconcile` checks `loft.trace.yaml`, a file beside `spec/` that links each acceptance
  criterion, and each Constraint and Quality Attribute whose `verificationFate` is `proved`, to
  the tests that verify it by file and name. Each link holds the content hash its node had when
  a reviewer last accepted it. `TR-001` reports a node no link names, `TR-002` a link never
  reviewed, `TR-003` a link reviewed before its node changed, `TR-004` a test that cannot be
  found as a whole word in its file, and `TR-005` a link whose node is gone or not in scope. No
  test is run. `--fail-on` gates CI; `--accept <key|label>` and `--accept-stale` record a review.
  The trace file enters no root, and no publish writes it.
- The `loft-spec` skill has a new reference, `reconciling.md`, and ends a change to a specified
  product by linking its tests.
- This repository traces its own criteria: its trace file is seeded from the test names that cited
  labels before test names stopped citing them, and CI runs `loft reconcile --fail-on warning`.
- A new lint rule, `V2-009` (info), reports each Markdown construct in a body outside the Format's
  subset of paragraphs, lists, code spans, fenced code blocks, emphasis and links: raw HTML, a
  block quote, a table, an image, a thematic break, an indented code block, a footnote, a task
  list item or struck-through text, each at its first body line.
- `loft import v1` converts a v1 tag whose name is not a v2 tag name, such as "MCP tool": the
  name is lowercased and each run of other characters becomes one hyphen (`mcp-tool`), for the
  Facet Tag and its Vocabulary Entry alike. A name that derives nothing, or two v1 names that
  derive one tag name, stop the conversion naming them. The sidecar keeps each v1 name that
  changed, under `tagNames`.
- A command whose standard output is closed before it finishes, as by `loft reconcile | head -1`,
  no longer panics with "failed printing to stdout: Broken pipe" and exit 101: it skips the rest
  of its output, writes nothing about it to standard error and exits with the code it exits with
  when its output is open. Any other failure to write standard output exits 1 with its message on
  standard error.
- `loft merge-driver` merges every record of a file by its key, wherever each branch holds it:
  its fields, its place (its parent and list) as one value, and each list in merged order. A
  record moved to another parent in the same file, such as a criterion moved to another
  Requirement or a Constraint moved between Components, is written once, where it was moved and
  with the other branch's edits. It used to stay at its old parent with a `record` conflict and
  also appear at the new one when the other branch edited it, appear twice when both branches
  moved it apart, and lose a deletion made in the other branch.
- A record the two branches moved to different parents stays at its base parent, or where `ours`
  put it when neither branch holds that parent, with a new conflict over `parent`, each side
  naming the parent's key. So does each record of a cycle the two branches' moves would make, and
  a record both branches added at different parents goes where `ours` put it, with a `parent`
  conflict. `parent` is reserved beside `conflicts`, `position` and `record`.
- A record one branch deleted is kept, as the other branch wrote it and with a `record` conflict,
  when the other branch changed or moved it or when the merge keeps a record beneath it. One the
  other branch changed only by deleting records beneath it is deleted.
- A record that a list the Format sorts moves, as renaming a tag moves its neighbours, is not
  changed by the move: renaming a tag next to one the other branch deleted no longer keeps the
  deleted tag with a `record` conflict.
- An assessment one branch deleted and the other changed keeps its base value with a conflict
  over `assessments`. It used to be dropped silently.
- A record whose kind one branch changed takes that kind, and its fields that both kinds define
  alike merge as before; an edit to a field the new kind lacks is dropped. Where the new kind
  cannot hold a record the merge keeps beneath it, the record keeps a kind of another branch that
  can, and the change of kind is dropped. Neither drop writes a conflict. Such a merge used to
  panic in a debug build.
- A merge input that carries a key twice is refused as unreadable, naming both sites, and a merge
  whose result would carry a key twice writes nothing.
- A key carried twice that a `record` conflict names, as when one branch moved a record to
  another file and the other changed it, is reported as a record moved in one branch and changed
  in the other.
- An error or finding that names two sites in one file names the file once:
  `spec/a.yaml:18:10, 28:10: …`.
- A Feature listed by two Feature Groups is one error that names both groups.
- A publish that settles a key which lost its live label raises the label counter above the
  key's withdrawn labels, so none of them is given out again.
- A `loft skill install` that fails leaves the skill directory as it was, with no temporary
  directory beside it.
- The skill's listing-budget refusal names the combined length and the limit.
- The `loft-spec` skill's format reference describes every record kind with its fields, each
  marked required or optional, and names every command that writes each file; tests now hold
  the reference to the code.
- CI builds `loftspec` with the toolchain its declared `rust-version` names.
- The release workflow's check job runs `loft reconcile --fail-on warning`, as CI does.
- Library API: `loftspec`'s `RuleId` gains `V2009`; `ConvertError::Tag` gives way to
  `UnderivableTag` and `TagCollision`; `Problem::ListedByTwoGroups` names its groups;
  `VocabularyEntry.name` is a `String` and `V1Sidecar` gains `tag_names`; `write_sites` is new;
  `MergeError` gains `Unplaced`, and `Problem` gains `MovedAndChanged`.
- Telemetry, on by default: each run of `check`, `find`, `fmt`, `import v1`, `init`, `publish`,
  `reconcile`, `render`, `skill install` and `verify` sends one pseudonymous command event to
  `https://askloft.ai/api/telemetry` after its output, within 500 milliseconds and printing
  nothing. A notice on standard error before the first event says what is sent and how to
  switch it off. `loft telemetry off|on|status` and `LOFT_TELEMETRY=off` switch it off;
  `schemas/telemetry-event.json` lists every field. See "Telemetry and the update check" in
  the README.
- The update check: at most once a day `loft` asks GitHub for the latest release, and a later
  run tells of a newer one with the way to update. `loft update-check off|on|status` and
  `LOFT_UPDATE_CHECK=off` switch it off.
- `loft record-install npm|cargo|release-binary` records how `loft` was installed. The npm
  package runs it from a `postinstall` script, which never fails an install; without a record,
  `loft` infers the way from where it lies.

## 2.0.3 (2026-10-14)

- The npm packages run again: 2.0.2's platform packages shipped `loft` without its execute
  bit, so `npx loft` failed with `EACCES`. The release now packs each package into a tarball
  before the binaries leave the job that marks them executable, runs `loft` from the tarballs,
  and publishes those tarballs. npm 2.0.2 is deprecated.
- `loft init` and `loft skill install` ask for the skill directories they wrote to be committed,
  so that every clone's coding agents have the skill; the `loft-spec` skill commits them with
  `.gitattributes` when it starts a document.
- The `loft-spec` skill keeps what the product leaves out of scope out of Constraints and
  criteria, recording it in a decision or in the body of the node it bounds.

## 2.0.2 (2026-10-14)

- A Product may hold Constraints, for a rule the whole product keeps. The v1 converter carries
  a Constraint under a Product across. A document that uses this is refused by an older `loft`.
- `loft fmt` turns a reference written by name (`name: "…"`) into its target's key: an option by
  title, a driver by label, a decision by title, a Requirement by its statement, any other node
  by its name. An accepted decision can be written in one pass.
- `loft fmt` drops the label binding of a deleted node, which `loft check` refused before, and
  reports each binding it drops; the counters keep the label from being allocated again. A
  retired or superseded decision's reference to the deleted node becomes its former label.
- `loft find <query>` prints the key, label, kind, file and name of each node or decision whose
  label, slug or key equals the query or whose name contains it.
- `LQ-009` matches a hyphenated tag such as `event-details` only in its hyphenated form, not in
  ordinary prose with the same words; a recorded divergence from v1.
- `loft init` suggests `loft merge-driver --install` inside a git repository that has not
  registered the merge driver, and suggests putting the project under git first outside one.
  `loft merge-driver --install` asks to commit `.gitattributes` when it writes it.
- The `loft-spec` skill finds `loft` through npm's runners, asks before `git init`, installs the
  merge driver, keeps interviewing until no answer would change the outline, keeps open questions
  out of `spec/`, writes decisions in one pass by name, corrects an unpublished decision in place
  and supersedes a published one only after asking.
- The README says how to work: write the specification with an agent, and read what it wrote,
  and installs from crates.io with `--locked`, so that the dependencies are the ones released
  together.
- The `loftspec` library's Rust API changes: `Product` gains `constraints`, the record kinds,
  scalar shapes and read problems gain variants, and a reference scalar names its `Target`. Those three enums are now `#[non_exhaustive]`,
  so that a variant added later breaks no `match` outside the crate. `loft-cli` 2.0.1 still
  builds against this release.

## 2.0.1 (2026-10-13)

- `@ask-loft/loft` on npm: prebuilt `loft` binaries for macOS, Linux and Windows on x64 and
  arm64, run by a launcher with no install script.
- Each release publishes from its tag, after the maintainer's approval, to crates.io and npm by
  Trusted Publishing, with the binaries attached to a GitHub release.

## 2.0.0 (2026-10-13)

The first public release. Version 2 because it succeeds the format of Loft v1, whose exports
`loft import v1` converts.

The Loft Specification Format, `loftspec/1`:

- A specification as a `spec/` tree of YAML files in a git repository: Products, Feature Groups,
  Features, Requirements in EARS forms with their acceptance criteria, Components, Constraints,
  Quality Attributes and architecture decisions.
- Every node carries a key; publishing gives nodes labels such as `RQ-12`, with aliases and
  withdrawn labels kept in `spec/labels.yaml`.
- Canonical hashing, `sha256-jcs/1`, into content, binding and state roots, pinned by
  `loft.lock.json`.
- Structural, language and writing rules, among them a compound criterion (`V2-005`), an
  oversized Requirement (`V2-006`), an obligation in a body (`V2-007`) and an article before a
  proper name (`V2-008`); a three-way merge over keys that writes typed conflict records,
  and the settling of two publishes that meet.

The command line:

- `loft init`, `fmt`, `check`, `render`, `import v1`, `publish`, `verify`, `merge-driver`,
  with `--install` and `--command` for a `loft` started by a dev-dependency launcher, and
  `skill install`.
- `--format json` on every command, each document described by a schema under `schemas/`, at
  version 1. Every document carries `schema`, `outcome` and `errors`; on an error it holds only
  those.
- Exit codes 0 on success, 1 on an error, a refusal or a failed check, and 2 on a usage error.

The agent skill:

- `loft-spec`, which teaches a coding agent to write, change and review a specification with
  `loft`: a short router and a reference for each workflow, with examples that run as tests.
- `loft init` writes it for Claude Code and Codex beside `spec/`, unless `--no-skill` is given;
  `loft skill install` writes it into any project.
- A plugin `loft` for each host, in this repository's own marketplaces.
