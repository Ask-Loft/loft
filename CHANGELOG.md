# Changelog

Releases of the `loft` command line and the `loftspec` and `loftspec-store` libraries. The
Format and the hashing algorithm carry their own versions, which `loft --version` prints.

## Unreleased

- Tasks can be assigned to agents independently of stage ownership, including parallel subagents
  within a main agent's stage. Claude task metadata and Codex plan-step prefixes bind native
  progress to existing tasks. Reassignment guards protect progress; observed ends return active
  work to open. The page shows assignees and separate execution attempts, and activity and tokens
  follow the assigned task's stage where unambiguous without duplicating elapsed time.

- Both dogfood launch commands rebuild Loft and restart the shared Yard server on port 7412
  before starting the agent, including same-version rebuilds. Concurrent launches serialize;
  existing sessions reconnect, and a failed build leaves the running server alone.
- Yard's token details start collapsed while summary totals stay visible, with the fold choice
  remembered separately for each Yard Log through live updates and reloads.
- Yard's free-form answers have explicit radio buttons or checkboxes. Typing selects the
  option, clearing text keeps it selected, and switching away retains text without sending it.
- Yard's waiting panel preserves mixed question, plan and approval order, and same-second
  submissions follow their local arrival order across callers.
- Claude captures tasks, questions, plans and Yard tool calls in receipt order for each caller,
  selects task stages before native creation and releases capture on interruption.
- Yard cards distinguish their Created and Updated times while keeping their stored timeline
  positions through edits, mode grouping and reversing the stage view.

- Codex Yard observes latest-request token usage from an existing local app server during
  identified native turns. Cached input is separated from fresh input, missing counters and
  observation gaps stay unavailable, and reconnects import no historical totals. Stable event
  identifiers prevent duplicate charges after delivery retries or page-server restarts.
  `loft yard codex watch --app-server-socket ABSOLUTE_PATH` selects a custom local socket;
  otherwise it uses the daemon socket under `CODEX_HOME` or `~/.codex` on Unix platforms.
- The Codex plugin includes its Yard skill, composed from the shared workflow and Codex
  instructions, and native hooks for local sessions. Questions and plans use the Yard page;
  Codex's native modes and permissions stay authoritative. Review and trust the hooks after
  installation. Native hooks provide activity; live usage comes from the local app server.
- `loft yard codex hook` translates native Codex activity and mirrors successful `update_plan`
  calls into the caller's active stage. `loft yard codex watch` owns session heartbeats, bounded
  event delivery and page-server recovery. Unavailable usage and tool outcomes remain unknown.
- `loft skill install --host codex|claude-code|all` selects the hosts to install, retaining both
  as the default. Codex-only installation leaves Claude Code files and settings alone. This
  repository exposes its canonical skill source through a Codex discovery link, which the
  installer preserves and the packaging guard checks; invalid skill links are refused.
- Yard has a host-neutral core and a native Claude adapter. Session identity includes the
  host across ownership, continuations, liveness, event storage, stage accounting and the page.
  Yard protocol 2 reports adapter capabilities and distinguishes unreported usage from zero;
  incomplete totals are marked partial. The Claude adapter retains protocol 1 compatibility.
  The Yard skill is composed from a shared workflow and Claude-specific instructions.
- The Claude Code plugin no longer shows the Loft status pane: its pane, its status-line entry
  and `/loft-status` are gone, and no session runs `loft` every 10 seconds to draw them. Yard Log
  and its page show the work on a specification instead. `loft status` stays a command, and the
  `baseBranch` and `loftCommand` settings stay for Yard Client.
- Yard Client: each branch's Yard Log, the record of what agents planned, asked, decided and did
  on it, kept in the `yardlog` format under `.yard/<branch>/`.
  - `loft yard` reads and writes it: `read`, `write` (with `--send` to send questions, approvals
    and plans), `wait` (holding a lease on the submission, at most 540 s at a time, and taking up
    a submission whose session is gone), `withdraw`, `close`, `published`, `answer`, `continue`,
    `protocol`, `upgrade`, and `serve`, the page server on the loopback interface. Each acts for
    a session and, for a subagent, its agent; Yard Log's root records the base branch it is
    merged into (the one `write --base` names, else the local branch git's reflog says the branch
    was cut from, else `--default-base`, else `develop`), and a read marks the revision read only with `--mark-read` and gives each `loft`
    block's report by its size unless `--reports` is given; a write is applied whole or not at all, under a lock in
    git's own directory; a `loft` block records a view of the specification's work from the
    same report `status`, `check` or `reconcile` gives.
  - `loft merge-driver` merges the files under `.yard/` record by record, and `--install`
    routes them to it; the next `loft yard` command settles what spans files.
  - The Yard page, served by `loft yard serve`: every Yard Log of the trees on the machine,
    live, with its stages and blocks, a Gantt chart, Loft's views, the tokens each stage took
    and what waits on the person, who answers and approves there. Mermaid ships with it,
    compressed.
  - The Loft plugin for Claude Code runs Yard Client in its mod: it adds the
    `yard_*` tools, puts `AskUserQuestion` and `ExitPlanMode` on the page beside Claude Code's
    own dialog, mirrors tasks, records each session's events and tokens, keeps one page server
    for the machine, and gives each session where its Yard Log stands. The `yard` skill teaches
    the agent to keep Yard Log. The plugin's new setting `yardPort` names the page's port.
  - A task has a type, `regular` or `fix`: a fix is work the person asked for to correct what was
    already done. The agent gives it in a task's metadata (`metadata: { type: "fix" }`), and the
    page marks each fix in the timeline and draws it in a tone of its own in the Gantt chart.
  - The `yard` skill has the person approve a change to the specification before it is committed
    and its build planned: the agent records its `spec-change` view over the working tree and
    sends an approval right after it, revising without a commit until the person approves. A
    requirement changed once the build's plan is approved is planned again only in the steps it
    touches.
  - New crates: `yardlog`, Yard Log's model; `yardlog-store`, its store; `loft-common`, the
    strict YAML subset, the key and the UTC time, which `loftspec` re-exports where it always
    had them; and `loft-ports`, the journaled save, the runner of git and the clock, beneath
    `loftspec-store`, whose API is unchanged.
- Settling a conflict of a document held in memory. `loftspec::conflict::settle` takes a
  document, a conflict's identifier and a `Settlement` (`Base`, `Ours`, `Theirs`, or
  `Value(ConflictValue)`) and gives a new document without that conflict record, as the reader
  reads it: a field takes the side's value, or the value given, checked against the field's
  shape; a conflict over the bindings adds the side's bindings to those the labels file holds;
  a conflict over `record` keeps the record or deletes it with everything beneath it, departing
  their bindings, removing every edge that names what it deleted, naming a node a retired or
  superseded decision references by its former label, and taking the deleted records' conflict
  records with them; a conflict over `parent` or `position` moves the record where the side puts
  it. Those three take a side only. Every refusal is a `SettleError`, including the reader's
  errors for a settlement whose document would not read. `codec::conflict_owner` gives the owner
  of the conflicts a record holds, and `ConflictOwner::own_record` the kind and key of the own
  record view it names.
- Functions on a document held in memory, without writing it and reading its files back.
  `loftspec::check::validated` validates a document at the sites its caller gives, without
  linting it, and `check::linted` lints it under the bytes of its `spec/lint.yaml`, without
  validating it, or gives the configuration's errors; for a document as the reader reads its
  written tree, each gives what `check` gives that tree. `codec::sites` gives the sites of the
  keys the files the writer writes carry, and `codec::write_sited` those files with them, without
  reading the files: the writer records where each key's value lands as it writes it.
  `publish::ReadTree::of_document` builds a publishable tree from a document and the text of its
  `spec/lint.yaml`: the tree `ReadTree::read` reads from the written files, its document in the
  order the reader reads it, or the same read errors. A document whose records hold only what the
  strict reader accepts is resolved from the records the writer builds, without parsing its
  files.
- `loftspec::codec::own_record` gives one record's own record view and digest by its
  `RecordKind` and key, without building any other record's: in a document that carries each key
  once, it is the entry `own_records` gives that record. A node is found by a walk that stops at
  it, with only the edges it writes; a binding by the key of the element it binds, departed or
  not. A kind and key no record carries give none.
- `loftspec::bindings::Bindings::record` records a binding whole, as `spec/labels.yaml` holds
  it: its label, kept where its prefix is no longer its element's kind prefix, as a promoted
  record keeps its label; its aliases and withdrawn labels in order; its departure; and the state
  and history a publish recorded in it. Recording each binding a labels file holds gives the
  bindings reading the file gives, which the reader now builds the same way. A binding of an
  element bound already, or of a kind that takes no label, is refused and changes nothing.
- `loftspec::ext::from_json` reads extension data back from the JSON `ext::to_json` writes it
  as, its exact inverse: it refuses, as a `FromJsonError` naming the JSON Pointer of the value
  and its `JsonProblem`, JSON that is not an object of namespaces, `null`, a number with a
  fraction or an exponent, an integer beyond ±9007199254740991, an empty array or object, and a
  mapping key that is not a letter followed by letters and digits, none of which the text form
  holds. `canonical::Digest::from_bytes` makes a digest from its 32 bytes, which `as_bytes`
  gives back, and `publish::PublishedAt` is ordered by the instant it names.
- The tree merger's result always reads. Where one side deletes a record and leaves it otherwise
  as it was, while a record the merge keeps references it, `merge_tree` keeps the record as the
  other side holds it, with every record beneath it, and gives it a conflict record over `record`
  whose deleting side is `null`, named by the identifier its holder, field and sides give it; a
  record it references is kept by the same rule. Before, it deleted the record and the result
  named a key that no record carried. Where both sides changed a field apart and its base value
  names a record both deleted, the field holds ours' value beside its conflict record. The merge
  driver, which sees one file, still deletes such a record and leaves the reference for
  `loft check` to report. A record that no place in the result can take, because a side changed
  the kind of its parent, is not kept, and `merge_tree` now refuses with
  `TreeMergeError::Unwritable` any result whose references do not resolve, where before it
  returned it. `MergedTree::document` is the document the result reads into, so a caller need
  not read the merged files again.
- `loftspec::codec::own_records` gives each keyed node, decision, registry entry and binding of
  a document its own record view and digest, by `RecordKind` and key: a stable identity of the
  record's own content, for tools that store or compare records one at a time. The view is the
  record as the writer writes it in canonical layout, as a JSON object of its fields, without its
  key, the records it holds and its conflicts; a binding is keyed by the key of the element it
  binds. `own_record_digest` digests a view under the new domain `loftspec/own-record/1`. Neither
  a record's children, its place among its siblings, its parent nor its key moves its digest, and
  no root holds it, so no content root, state root or golden vector changes.
- A command event's `time` is the hour of the system clock. It no longer follows
  `SOURCE_DATE_EPOCH`, which names the time of a publish, so a build or a test that fixes that
  time no longer sends events dated to it.
- With `--format json`, each read error of the tree names the reader's problem by its stable kind
  in `problem` (`syntax`, `unknown-field`, `duplicate-key`, …), beside the `kind` `read`; an
  error of any other kind has none. The schemas of the commands that read the tree state it,
  and those a release published name a new version: `loft-find/2`, `loft-fmt/2`,
  `loft-reconcile/3`, `loft-render/3`, `loft-status/2`, `loft-terms/2` and `loft-verify/2`;
  `loft-check/2`, `loft-publish/2` and `loft-review/1`, not yet released, state it too.
- `loftspec::edit` edits a document held in memory by key. `find` gives the node or decision a
  key names. `detach` takes a node out with every node beneath it and the edges they write, as a
  `Subtree`; `insert` puts a subtree under a parent, a node or the document's root, at a position
  in its list of nodes of that kind; and `move_node` moves a node, keeping its key and its
  subtree. A Component put at the root without a slug takes the one given with the root,
  `Parent::Root(Some(slug))`, and a root Component put under a Component leaves its slug. Each
  edit refuses, with a typed `Refusal` and the document unchanged, a key the document does not
  hold, a kind the parent cannot hold, a key it already carries, a position past the end of the
  list, a move beneath the node itself, a Component at the root with no slug and none given, and
  a node that would be written in a file another node holds, naming the file and its holder. No
  edit sets a slug its caller does not give: `codec::write` writes each node where its new parent
  and its slug place it. `kind::ParentKind` names what a node stands under, and `Index::carries`
  whether any element or entry carries a key.
- `loftspec::review::page` gives a page of the review packet of a tree and a base held in
  memory, each a document with the sites of its keys: the units as JSON, every next whole unit
  that fits under a limit of characters, a longer unit alone and marked `oversized`, with the
  page's number and the number of pages. It refuses a tree that holds a conflict record or a node
  without a key, and a page past the last. `loft review` prints the page it gives, so its output
  is unchanged; `loftspec::codec::site_fields`, `Site::fields` and
  `loftspec::query::element_fields` give the JSON fields of a site and of an element.
- `loftspec::diff::compare` compares two versions of a document held in memory, each with the
  text of its `spec/lint.yaml`, matching nodes and decisions by key. Each node is reported added,
  changed, moved, removed or reordered, and each decision added, changed or removed, with its
  whole record in each version that holds it, as the text form writes it; a node's record leaves
  out the nodes beneath it, whose order is reported apart. The Products' order, and which of the
  tag and category registries, the term list, the lint configuration and the bindings differ,
  are reported too. A version that holds a node without a key or a conflict record is refused.
- Two lint rules of decision history, both warnings that refuse no publish. `V2-010` reports a
  decision published as accepted, rejected, superseded or retired whose meaning has changed
  since, judged by its decision history hash against the `publishedHistory` its binding records,
  so an edit to a node it names is no edit to it; a decision superseded or retired since it was
  published as accepted is compared as if still accepted. `V2-011` reports a change of state that
  the recorded `publishedState` does not allow (only a proposal may be accepted or rejected, and
  only an accepted decision superseded or retired) and a resolved decision deleted rather than
  retired. Both read only `spec/labels.yaml`, never a repository's history; `RuleId` gains
  `V2010` and `V2011`, and the schema `loft-check/2` lists both.
- `loftspec::check::lint_against` lints a tree and a base, two documents held in memory, under
  the tree's lint configuration, and splits the tree's findings against the base's as
  `loft check --baseline` does, with no path and no repository; `check_against` runs it over two
  trees read from their files.
- `loft publish --format json` writes the schema `loft-publish/2`, whose `givenBack` lists each
  label a publish gave back to a key that came back to its document, with its key. A connected
  document is published only through its remote, so `loft publish` refuses one.
- A lock may name the state view its state root was computed under, in `stateView`. State view 2
  adds the text of `spec/lint.yaml`, digested under `loftspec/lint/1`, to the state root, so a
  lock proves which lint configuration a state was published with. A tree without the file has
  the same state root under both views, and its lock names none. `loft publish` writes view 2 for
  a tree that has the file, so the first publish of such a tree after upgrading advances its
  publish sequence once. `loft verify` checks each lock under the view it names, so every lock
  published before still verifies, and refuses a lock that names a view it does not compute,
  naming that view and the highest it computes. `StateView` names the views.
- `spec/loftspec.yaml` may say `connected: true`: the document's labels and locks come from its
  remote. The field enters no root.
- A key that leaves the tree keeps its binding: `loft fmt` and `loft publish` mark it `departed`,
  with its labels and the counters as they were, so its labels are never given to another key. A
  retired or superseded decision that names such a node names it by its former label. When the
  key comes back, the publish gives it its label back, listed in `givenBack`, and the decision
  names it by its key again. A publish that would give back a label a decision still names under
  `formerLabel` is refused, and `loft fmt` settles it. At each publish a decision's binding also
  records `publishedState` and `publishedHistory`, which the decision-history rules read. None of
  these fields enters a root, and a binding's departure and records merge apart from its label.
- `loftspec::publish::publish` takes the repository state as an option, so a tree held in memory
  publishes without git, and says whether the publish is made as the document's remote:
  `Made::AsRemote`, the one publisher that allocates a connected document's labels, or
  `Made::NotAsRemote`. It gives `Publication::Published`, with the lock, the files to write, the
  labels given back and allocated, and the `Base` it was made after (sequence 0 and no state root
  without a previous lock); `Unchanged`, with the previous lock; or, for a connected document's
  publish not made as its remote, `Prepared`: the validated tree, its roots and its state root,
  and no lock. `publish_labels` and `record_decisions` give two of its steps apart.
- `loftspec::merge::merge_tree` merges three whole `spec/` trees, pairing records by key across
  files: a record moved to another file on one side and changed on the other comes out once, in
  the file the moving side gave it, with the change, and a record the two sides moved to different
  places is a `parent` conflict. The git merge driver still merges one file at a time.
- Each conflict record is named by an identifier, `ConflictId`: SHA-256 under
  `loftspec/conflict/1` over its holder, its field and its three sides, each side as the conflict
  record writes it, every scalar as text. It is derived, never stored, so two merges that write one
  conflict name it alike, and resolving another leaves it as it is. The validator lists a tree's
  conflict records in document order and names each by its identifier; `Conflict::id` gives it,
  and `Conflict::view` the JSON it is computed over.
- `loftspec::hash::decision_history_hash` and `decision_history_view` give a decision's history
  hash, under `loftspec/decision-history/1`, and the view it is computed over. No root holds it.
- A Feature Group may hold Quality Attributes, for a quality its Features share; its record
  writes them after its Constraints. A Product holds none. A Feature Group without them keeps its
  content hash, so the Format stays `loftspec/1`: every tree written before reads and hashes as
  before, and a tree that uses this is refused by an older `loft`. The v1 converter carries a
  Quality Attribute under a Feature Group tier section across, and still refuses one under a
  Product or a Requirement, and a Constraint under a Feature, naming both nodes.
- `loftspec::convert::v1::convert_keyed` converts a v1 export under a document key the caller
  names. An export with no node, registry entry or decision, as a document's first published
  state can be, becomes an empty document under that key, derived from the export's state; an
  export whose elements carry another document is refused as `OtherDocument`, naming both.
  `convert` still refuses an empty export, which names no document.
- `ConvertError::kind`, `ExportError::kind` and the reader's `Problem::kind` name each error by a
  kind that stays the same from release to release (`containment`, `missing`, `no-document`,
  `duplicate-key`, …; `syntax`, `eof` or `data` for an export that is not a v1 export), so a tool
  that runs the converter or the reader over many inputs can count what went wrong by kind.
- The library also gives `loftspec::diff::content_changes`, the added, changed and removed nodes
  without the moved, which spares indexing both trees; `loftspec::query`, how a key, slug or
  label names an element, which `loft find` and `loft review --node` share, so a key now names a
  node in any case for both; `Finding::identity`, what makes a finding of one tree the same as one
  of another; and `DecisionState::outcome`.
- `loft init --no-plugin` and `loft skill install --no-plugin` write the skill and leave
  `.claude/settings.json` as it is: not created, not read, and not named in the request to
  commit. It suits a project that runs a skill built from source rather than the released
  plugin. `loft init` refuses `--no-plugin` with `--no-skill` as a usage error.
- `loft check --baseline <revision>` lints the tree at the commit where HEAD left the revision
  too, under the working tree's `spec/lint.yaml`, and marks each finding `introduced` or
  `preexisting`; the base's findings the tree no longer holds are listed as `resolved`. A
  finding is the same at both sides when its rule, subject keys and term are, so a moved node or
  a changed count in a message is no new finding, and repeated findings pair by body line first.
  `--fail-on` then gates on introduced findings only, and the text prints only those, then a
  count. The output follows the schema `loft-check/2`: each finding may carry `change`, and the
  document `resolved` and `baseline` (the revision, the commit, the `loft` version and the
  digests of both lint configurations, and whether they agree). A base it cannot read is an
  error of the new kind `base`.
- `loft review [--base <revision>] [--node <node>] [--max-chars <n>] [--page <k>]` gathers the
  context each assertion (a Requirement, criterion, Constraint or Quality Attribute) is read
  against, and judges nothing. Each assertion lands in the unit of its scope, the Feature or the
  node that holds it, with the nodes above the scope and their Constraints and Quality
  Attributes, a Feature's Requirements with their criteria, the Components it realizes with
  those above them, the decisions that name, are driven by, bear on or mention an assertion or
  the scope, and the terms the assertions use. With `--base`, only the assertions the branch
  added, changed, moved or removed are reviewed, each beside its node at the base. `--node`
  narrows the review to a subtree by key, slug or label. The packet is JSON, paged by whole units
  of at most 50000 characters unless `--max-chars` says otherwise, and the text output is an
  index of the page. It follows the schema `loft-review/1`, and the command event names
  `review`.
- The `loft-spec` skill asks for fewer approvals: within a specification task the user asked
  for, edits of `spec/`, the `loft` checks and reviews and commits on the working branch need
  none; questions that change the content are asked once, up front; and only a push, a merge
  into a shared branch, a publish, deleting a branch, rewriting history or superseding an
  accepted decision is asked each time. Its review of a change starts from `loft check
  --baseline` and `loft review --base`, adds precedence to the contradictions it looks for,
  disposes of each finding, and fixes a large review in waves. The writing rules add ownership
  apart from realization, guarantees rather than provisioning and the document's conventions;
  changing a product and publishing add their git practice; and a new reference, "Choosing the
  next change", finds and plans what to build next from the published states.

## 2.2.1 (2026-10-21)

- `loft reconcile` locates a linked test in a `.java` file in its syntax tree, parsed with the
  pinned tree-sitter Java grammar: a method or a class, at any depth of classes, interfaces,
  enums and records, with the annotations and modifiers written before it. Its digest is recorded
  as for any other located test, so a Java test rewritten after its link was accepted is reported.
  A Java test that was found only as a word before now reports `TR-010` where a part of its file
  that the grammar does not parse may cut it short.

## 2.2.0 (2026-10-18)

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
- `loft reconcile` locates each linked test in its file's syntax tree, parsed with tree-sitter,
  for Rust (a function, also inside a macro such as `proptest!`), TypeScript, TSX and JavaScript
  (a call to `it`, `test`, `describe`, `context`, `specify` or `bench`, also through `.only`,
  `.each(…)`, `.skipIf(…)` and the like), Python (a function or class with its decorators) and Go
  (a function, a method, or a `t.Run` subtest), in parentheses or not; an escape in a name is
  read as the language reads it, so a JavaScript surrogate pair is one character and Go's `\x` and
  octal escapes are bytes read as UTF-8. It
  records each located test's digest, the SHA-256 of its tokens without comments, as the test's
  `body` in the trace file, so a test rewritten after its link was accepted is reported. What a
  formatter changes does not count: whitespace between tokens, a comma directly before a closing
  bracket (except in Python, where `(x,)` is a tuple), a Python line broken inside brackets or
  after a backslash, CR LF line endings, and comments, also inside a macro. JSX text is read as
  JSX reads it, so spaces within a line count and the indentation around a line break does not.
  A line break that changes what JavaScript means, such as one after `return`, is not seen
  either.
  - `TR-006` (warning): a located test whose digest differs from its `body`; `--accept-stale`
    accepts it.
  - `TR-007` (info): a test found only as a word, in a file no grammar parses or under a name no
    test of the file carries, whose content is not tracked.
  - `TR-008` (info): a literal in backticks in the node's name that the source of none of its
    tests holds.
  - `TR-009` (warning): a test whose body was recorded but that is now found only as a word, as
    when it was renamed and its old name is left in a comment; accepting its link removes the
    body, after which the test is `TR-007`. `--accept-stale` accepts it.
  - `TR-010` (warning): a test that holds, or is followed before the next test by, a part of its
    file the grammar does not parse, given by line, column and text, or the token missing. The
    grammar may have cut the test short, so its link cannot be accepted until the file parses
    there; the other tests of the file are unaffected.

  Accepting a link records its tests' bodies, and any accept also records the body of every
  located test that has none. `--accept-stale` writes the trace file when it records a body even
  if it accepts no link, so a reviewed project adopts bodies with no new review; the output gives
  the number recorded.
- The trace file's format is now `loft-trace/2`, which adds a test's `body`; `loft-trace/1` is
  still read and is rewritten as `loft-trace/2` on the next accept. An older `loft` refuses a
  `loft-trace/2` file, so everyone who reconciles a project needs this release.
- `loft reconcile --format json` writes the schema `loft-reconcile/2`: the rules `TR-006` to
  `TR-010`, `recorded`, the number of bodies recorded, and a `tests` list with each linked test's
  node, file and name, whether it was located, and the first and last line of each location.
- A test file path that steps up and back inside the project, such as `tests/../tests/a.rs`, is
  found, and a test name is found as a whole word where it overlaps a part of a word. A path that
  names something other than a regular file, such as a named pipe, is `TR-004` at once instead of
  waiting on it.
- A specification can name its concepts in a term list, `spec/terms.yaml`: each term a `name`,
  a `definition` and the `aliases` the prose should not use for it. The file is optional, and a
  document without terms keeps its content and state roots. With terms, the content view holds
  a term list root after the decisions root, the JSON export a `terms` member, and the Markdown
  projection a Glossary after the Domain Vocabulary. An older `loft` refuses a tree with
  `spec/terms.yaml`, so everyone working on a project needs this release once it adds one.
- `LQ-010` (warning) reports an alias of a term in the name, statement or body of a node, in any
  case and with or without a final plural. `SH-011` (warning) reports a name or alias that two
  terms carry, or an alias equal to its own term's name. A project without terms sees neither.
- `loft terms [--format json]` prints the term report: groups of names that may denote one
  concept, for the user to decide in the term list, and the subjects of each Product's
  statements. `TD-001` finds one qualifier before head nouns of one noun group ("audit entry",
  "audit row"), `TD-002` one name written as one word and as two ("change set", "changeset", or
  an identifier such as `ChangeSet` written as prose), and `TD-003` one phrase with an
  abbreviation and in full ("tenant admin", "tenant administrator"). A group whose every name is
  a term or an alias is not listed again. The command exits 0 whatever it lists and writes
  nothing; its output follows the schema `loft-terms/1`, and its command event names `terms`.
  `loft check` never runs these rules.
- `spec/lint.yaml` configures `TD-001`..`TD-003` like the lint rules. The noun groups
  (`nounGroups`) and the abbreviations (`pairs`) are a new kind of option, named groups of words,
  edited by `replace`, or by `extend` and `remove`. The defaults are published with the other
  lint defaults, and the function words a phrase neither begins nor ends with as
  `testdata/golden/function-words.txt`.
- The `loft-spec` skill teaches the agent to read the term list before writing, to run
  `loft terms` after writing, and to put each group to the user and record the answer.

### Breaking for library users

- `loftspec`: `Document` gains `terms`, `Roots` gains `terms`, `Role` gains `Terms`,
  `ConflictOwner` gains `Term`, `Order` gains `ByText` and `RuleId` gains `Sh011`, `Lq010` and
  `Td001` to `Td003`. `loftspec::lint::report` holds the term report.

- `loftspec::trace`: `Test` gains `body` and is built with `Test::new`; `Reconciled` gains
  `found`, what was found of each test; `TraceRule` gains `Tr006` to `Tr010`; `FILE_FIELDS`,
  `LINK_FIELDS` and `TEST_FIELDS` are slices; `TestFiles` gains `tests`, which gives a
  `ParsedFile`, the tests a file holds, each with its `bytes`, and the `ParseError`s of the parts
  its grammar does not parse, none by default. `Found` has the outcomes `Located`, `Word`,
  `Unparsed` and `Missing`, and `Untracked::NoTest` carries the file's first `ParseError`.
  `TraceRule`, `Test`, `Reconciled` and the new `ParsedFile`, `LocatedTest`, `Lines`,
  `ParseError`, `Found` and `Untracked` are `#[non_exhaustive]`, so a later rule, field or outcome
  breaks no caller.

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
