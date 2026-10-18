# Golden vectors

These vectors are published with the Loft Specification Format so that a second implementation
can check its hashing byte for byte (AC-48). They were computed by an implementation independent
of the `loftspec` crate. A failing vector is a bug in the code or a deliberate format change,
never a reason to regenerate the file.

Each vector names a view, the domain string it is digested under, the view's RFC 8785 canonical
JSON, and the digest: SHA-256 over the domain string, one zero byte and the canonical bytes
(RQ-12). Digests inside views are 64 lowercase hexadecimal digits; keys are lowercase hyphenated
UUIDs.

## Views

Every view whose fields have an order is a JSON array, because RFC 8785 sorts object members.
Absent values are `null`; an empty string is not absent.

| View | Domain | Shape |
| --- | --- | --- |
| Node | `loftspec/node/1` | `[kind, text, body, tags, qaKind, qualityTarget, constraintBinding, conservationKind, verificationFate, boundary, children]` |
| Edges root | `loftspec/edges/1` | `[[sourceHash, type, targetHash, role], …]` |
| Registry root | `loftspec/registry/1` | `[registryName, [[name, definition], …]]` |
| Decision | `loftspec/decision/1` | `[title, context, state, drivers, options, assessments, consequences]` |
| Decisions root | `loftspec/decisions/1` | `[decisionHash, …]` |
| Content root | `loftspec/content/1` | `[productHashes, componentHashes, edgesRoot, tagsRoot, categoriesRoot, decisionsRoot]` |
| Binding root | `loftspec/bindings/1` | `[[key, label, aliases, contentHash], …]` |
| State root | `loftspec/state/1` | `[format, algorithm, documentKey, contentRoot, bindingRoot, derivedFrom]` |

### Node

- `kind` is the kind name: `product`, `feature_group`, `feature`, `requirement`, `criterion`,
  `component`, `constraint`, `quality_attribute` or `section`.
- `text` is a Requirement's statement rendered by its form's template, or any other node's name.
  A statement stored as `text` and the same sentence stored as slots give one view.
- `body` is the body byte for byte, or `null`.
- `tags` is the set of tag names sorted by their UTF-8 bytes, each name once.
- `qaKind`, `conservationKind` and `verificationFate` are their lowercase names or `null`;
  `qualityTarget` is `{metric, band, unit, aggregation, scenario, environment}` with `band` as
  `{kind: "at_most" | "at_least" | "exactly" | "between", upper?, lower?, value?}`;
  `constraintBinding` is `{category, requiredValue}`. Each is `null` when absent.
- `boundary` is a Component's `boundary` field as written, `private` or `external`, and `null`
  when the field is absent or the node is not a Component. A Component's inherited boundary is
  not part of its view.
- `children` is the children's content hashes, grouped by kind and in order within each kind: a
  Feature Group's Features, then its Constraints; a Feature's Requirements, then its Quality
  Attributes; a Component's Components, then its Constraints, then its Quality Attributes; and
  last, the node's Sections. The key, the label, the aliases, the node's position among its siblings and
  every confirmation flag are not in the view.

### Edges root

Each edge is its source's hash, its type (`realizes`, `scoped_to`, `mentions` or `subject`), its
target's hash and its role (`affected`, `outcome` or `null`). An endpoint's hash is a node's
content hash or a decision's view hash. The tuples are sorted by their canonical bytes and two
equal tuples both appear.

### Registry root

`registryName` is `tags` or `categories`. Entries are `[name, definition]`, sorted by name and
then definition, by UTF-8 bytes. Entry keys are not in the view.

The term list root view is `["terms", [[name, definition, aliases], …]]`, the aliases sorted and
the entries sorted by name, then definition, then aliases, by UTF-8 bytes. A document whose term
list is empty has no term list root.

### Decision

- `state` is `{kind: "proposed"}`, `{kind: "accepted", selectedOption, outcome}`,
  `{kind: "rejected", rationale}`, `{kind: "superseded", selectedOption, outcome, successor}` or
  `{kind: "retired", selectedOption, outcome, rationale}`. `selectedOption` is the option's
  position; `successor` is the successor decision's view hash.
- `drivers` keep their order: `{kind: "local", label, statement}` or `{kind: "subject",
  subject}` with the subject node's content hash.
- `options` keep their order as `[title, description]`.
- `assessments` are `[optionPosition, driverPosition, rating, rationale]`, sorted by option
  position and then driver position.
- `consequences` keep their order as `[kind, statement, subjectHash]`, the subject `null` when
  there is none.

No key of the decision or of its elements is in the view. The decisions root is the sorted list
of decision view hashes.

### Content, binding and state roots

`productHashes` are the Product content hashes in the manifest's order; `componentHashes` are the
root Component content hashes sorted. The content view is `[productHashes, componentHashes,
edgesRoot, tagsRoot, categoriesRoot, decisionsRoot]`, followed by the term list root when the
document has a term. Binding rows are sorted by key; `aliases` keep the order in
which they were recorded; `contentHash` is computed from the tree, never read from a file. The
state view's `format` is `loftspec/1`, its `algorithm` `sha256-jcs/1`, and `derivedFrom` is
`{authoredStateId, contentAddress, publishSequence}` or `null`.

## `small-document.json`

One Product, `Example Product` (body `The example.`, tag `portable`), holds the Feature Group
`Identity`, which holds the Feature `Keys` (body `Keys address nodes.`) and then a Constraint
with the fate `proved`. The Feature holds a Requirement stored as the ubiquitous form with
`system` `The writer` and `shall` `write a key on every node.`, which holds the criterion
``Every node record carries a `key` field.``, and then a latency Quality Attribute with a p99
target and the fate `sampled`. One root
Component, `Document model`, has a body with a tab, a trailing space and a final line break, a
Constraint with a technical binding (`runtime.language` = `rust`) and the fate `proved`, and a
latency Quality Attribute with a target and the fate `sampled`. Five edges: the Feature realizes
the Component, the Component is scoped to the Product, the criterion mentions the Requirement,
and the decision's two subject edges (`outcome` to the Requirement, `affected` to the Component).
Two tags, one constraint category, one accepted decision with two drivers (one local, one naming
the Requirement), two options, four assessments and two consequences. Ten bindings, the
Requirement's with one alias. Key `n` is `00000000-0000-7000-8000-` followed by `n` as twelve
hexadecimal digits; the document key is key 0.

## `lint-defaults.yaml`

The linter's default configuration, written as a `spec/lint.yaml` that states every default: each
rule's `enabled` and `severity`, the lexicons of `LQ-001`..`LQ-005` as `replace` lists, the
`LQ-005` conjunction threshold, and the `LQ-008` minimum token count and similarity threshold.
A tree without `spec/lint.yaml` is linted as if it held this file. `rules` and `suppressions` are
lists of records: a rule's record names it in `rule`; a suppression names `rule`, `key` and a
non-blank `reason`.

The language rules read text as tokens: a text splits on Unicode word boundaries (UAX #29), and
each piece that holds a letter or a decimal digit is a token, lower-cased, so `65,536`,
`read_graph` and `don't` are one token each and punctuation is none. A body is read as a
Markdown reader shows it, CommonMark without extensions: code blocks, HTML blocks and code spans
are left out, and the text pieces are joined by single spaces. `LQ-005` reads a body sentence by
sentence (Unicode sentence boundaries). `LQ-008` measures two names as the size of their tokens'
multiset intersection over the size of its union.

## `small-document.export.json`

The canonical JSON export of `small-document`, RFC 8785 bytes. Its top level is `format`,
`stateRoot`, `nodes` (each `{key, label, kind, view}` in document order, `label` `null` where a
node has none), `decisions` (each `{key, label, view}` by key), `edges` (each `{type, source,
target, role}` sorted by canonical bytes; `role` is `null` except on `subject` edges, and a
`subject` edge to a node the document no longer holds has the target `{formerLabel}`), `tags`
and `categories` (each `{key, name, definition}` by name), then `terms` (each `{key, name,
definition, aliases}` by name, its aliases sorted) only when the document has a term. Every
`view` is the content view or decision view the hasher digests.

## `function-words.txt`

The words a phrase of the term report neither begins nor ends with, one a line, sorted by UTF-8
bytes. The term report reads every run of one to three words between punctuation, code and
possessives, and leaves out a run that begins or ends with one of them.
