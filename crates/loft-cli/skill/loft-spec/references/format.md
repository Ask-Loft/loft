# The format

Use this workflow for the shape of what you write in `spec/`: which file holds each record, each
record's fields in order, the five statement forms, the edges, `spec/lint.yaml` and `loft.trace.yaml`. It owns the
fields and their values. The order of the steps, the YAML subset and the files `loft` owns belong
to `the-loop.md`; how to word a node belongs to `writing-rules.md`. Never restate or override
them here.

The examples form one small document, a reading list, and they are checked: written into a tree
that `loft init` created and formatted by `loft fmt`, they pass `loft check --fail-on warning`.
New nodes in them carry no `key`, as you write them. A node that another node references carries
a key that stands in for the one `loft fmt` would have minted; never copy it.

## Contents

- The files of `spec/`
- The manifest and the labels
- Products and Feature Groups
- Features and Requirements
- Statements
- Components and Constraints
- Quality Attributes
- Sections and bodies
- Extension data
- Tags and categories
- Decisions
- Edges
- Lint configuration
- The trace file

## The files of `spec/`

| Path | Holds | Written by |
|---|---|---|
| `spec/loftspec.yaml` | The manifest: format, algorithm, document key, Products in order | `loft init`; `loft fmt` adds each new Product |
| `spec/labels.yaml` | Label counters and the bindings of labels to keys | `loft init` and `loft publish`; `loft fmt` drops a deleted node's binding |
| `spec/tags.yaml` | The tag registry | `loft init`, empty; then you and `loft fmt` |
| `spec/categories.yaml` | The category registry for Constraint bindings | `loft init`, empty; then you and `loft fmt` |
| `spec/lint.yaml` | The lint configuration, optional | you |
| `spec/products/<product>/product.yaml` | A Product and its Feature Groups | you and `loft fmt` |
| `spec/products/<product>/<group>/<feature>.yaml` | One Feature, in its group's directory | you and `loft fmt` |
| `spec/components/<component>.yaml` | One root Component and everything beneath it | you and `loft fmt` |
| `spec/decisions/<decision>.yaml` | One decision | you and `loft fmt` |
| `loft.lock.json`, beside `spec/` | The lock that pins the published tree | `loft publish` only |
| `loft.trace.yaml`, beside `spec/` | The links from nodes to the tests that verify them | you; `loft reconcile --accept` writes `reviewed` |
| `v1-sidecar.json`, beside `spec/` | What a Loft v1 export held that the format has no field for | `loft import v1` only |
| `openspec-sidecar.json`, beside `spec/` | Where each node imported from an OpenSpec project came from, with the requirement and scenario names | `loft import openspec` only |

`loft fmt` writes each file it names in canonical layout. `loft publish` also rewrites each
citation of a label it withdrew, in the prose of the node or decision that cites it.
`loft import v1` writes a whole tree from a Loft v1 export, and `loft import openspec` one from
the main specs of an OpenSpec project; each writes its sidecar beside the tree.
`loft skill install` writes this skill beside `spec/`, in a directory for each agent, as
`loft init` does unless given `--no-skill`.

A file's name is its slug: lowercase letters, digits and hyphens. No other path under `spec/` is
allowed, but files and directories whose names start with `.` are ignored. A registry with no
entries is an empty file.

Fields are written in the order the tables below give; an optional field that is absent, or a
list that is empty, is left out. Each field is marked required or optional; `key` and `slug` are
required in a formatted tree, and `loft fmt` writes them. The names `conflicts`, `parent`,
`position` and `record` are reserved for the conflict records of a merge, which
`publishing-and-merging.md` settles; no record has a field of those names.

## The manifest and the labels

`loft init` writes both files, as the table above says; their fields are these.

| Manifest field | | `derivedFrom` field | |
|---|---|---|---|
| `format` | required: the Format's name and version | `authoredStateId` | required |
| `algorithm` | required: the hash algorithm | `contentAddress` | required |
| `documentKey` | required: the document's key | `publishSequence` | required |
| `publishBranch` | optional: the branch publishes run on, where it is not `main` | | |
| `products` | optional: the Products' keys, in order | | |
| `derivedFrom` | optional: the Loft v1 Authored State an imported document derives from | | |

| `spec/labels.yaml` field | | `counters` field | |
|---|---|---|---|
| `counters` | required | `P`, `FG`, `F`, `RQ`, `AC`, `CMP`, `CON`, `QA`, `ADR` | required: the next number of each label prefix |
| `bindings` | optional: by key | | |

| Binding field | |
|---|---|
| `key` | required: a node's or a decision's key |
| `label` | required: its live label |
| `aliases` | optional: the labels it carried before |
| `withdrawn` | optional: the labels withdrawn from it |

## Products and Feature Groups

| Product field | | Feature Group field | |
|---|---|---|---|
| `key` | required | `key` | required |
| `slug` | required | `slug` | required |
| `name` | required | `name` | required |
| `body` | optional | `body` | optional |
| `tags` | optional | `tags` | optional |
| `mentions` | optional | `mentions` | optional |
| `ext` | optional | `ext` | optional |
| `featureGroups` | optional | `features` | optional: Feature keys, which `loft fmt` writes |
| `constraints` | optional | `constraints` | optional |
| `sections` | optional | `sections` | optional |

A Feature Group's slug is the name of its Features' directory; `features` lists their keys in
order, and `loft fmt` adds each new Feature file to it.

```yaml path=spec/products/reading-list/product.yaml
name: "Reading List"
body: |
  A personal list of books to read.

  Each book moves from the shelf to the finished list.
featureGroups:
  - name: "Shelf"
    body: "Keeping the books still to read."
    constraints:
      - name: "The shelf shall hold at most 500 books."
        constraintBinding:
          category: "storage.limit"
          requiredValue: "500"
        conservationKind: cap
        verificationFate: proved
constraints:
  - name: "The Reading List shall keep every book in the reader's own storage."
```

## Features and Requirements

| Feature field | | Requirement field | | Criterion field | |
|---|---|---|---|---|---|
| `key` | required | `key` | required | `key` | required |
| `slug` | required | `statement` | required | `name` | required |
| `name` | required | `body` | optional | `body` | optional |
| `body` | optional | `tags` | optional | `tags` | optional |
| `tags` | optional | `mentions` | optional | `mentions` | optional |
| `realizes` | optional | `ext` | optional | `ext` | optional |
| `mentions` | optional | `criteria` | optional | `sections` | optional |
| `ext` | optional | `sections` | optional | | |
| `requirements` | optional | | | | |
| `qualityAttributes` | optional | | | | |
| `sections` | optional | | | | |

A criterion's `name` is the criterion: one sentence that a test or a reader can check.

```yaml path=spec/products/reading-list/shelf/add-a-book.yaml
name: "Add a book"
body: "Putting a book on the shelf, by its title."
tags:
  - offline
realizes:
  - 00000000-0000-7000-8000-000000000001
requirements:
  - key: 00000000-0000-7000-8000-000000000002
    statement:
      form: event_driven
      when: "the reader adds a book by its title"
      system: "the shelf"
      shall: "hold the book."
    criteria:
      - name: "The book appears on the shelf."
      - name: "A book already on the shelf is not added twice."
  - statement:
      form: unwanted
      if: "the title is empty"
      system: "the shelf"
      shall: "refuse the book and name the empty title."
    criteria:
      - name: "The shelf is unchanged."
      - name: "The message names the empty title."
qualityAttributes:
  - name: "The shelf shall show an added book within 200 milliseconds at the 95th percentile."
    qaKind: latency
    qualityTarget:
      metric: "shelf.add.latency"
      band:
        kind: at_most
        upper: "200"
      unit: "ms"
      aggregation: p95
      scenario: "Adding one book to a shelf of 500 books"
      environment: "A mid-range phone"
    verificationFate: sampled
sections:
  - name: "Why titles"
    body: "Readers remember a book's title better than its author."
```

## Statements

A Requirement's `statement` is a mapping: `form`, then the form's condition slot, then `system`
and `shall`. The `shall` slot holds everything after "shall", its full stop included.

| Statement field | |
|---|---|
| `text` | optional: a whole sentence, in place of `form` and its slots |
| `form` | optional: `ubiquitous`, `event_driven`, `state_driven`, `unwanted` or `optional`; required without `text` |
| `when`, `while`, `if`, `where` | optional: the condition slot its `form` takes |
| `system`, `shall` | optional: every `form` takes both |

| `form` | Slots in order | Renders as |
|---|---|---|
| `ubiquitous` | `system`, `shall` | <system> shall <shall> |
| `event_driven` | `when`, `system`, `shall` | When <when>, <system> shall <shall> |
| `state_driven` | `while`, `system`, `shall` | While <while>, <system> shall <shall> |
| `unwanted` | `if`, `system`, `shall` | If <if>, then <system> shall <shall> |
| `optional` | `where`, `system`, `shall` | Where <where>, <system> shall <shall> |

A statement may instead be `text:` alone, a whole sentence; lint asks for the slots (`V2-002`).
`writing-rules.md` says which form fits which condition.

## Components and Constraints

| Component field | |
|---|---|
| `key` | required |
| `slug` | required on a root Component, absent on a nested one |
| `name` | required |
| `body`, `tags` | optional |
| `boundary` | optional: `private` or `external` |
| `scopedTo` | optional: Product keys |
| `mentions` | optional |
| `ext` | optional |
| `components` | optional: nested Components |
| `constraints`, `qualityAttributes`, `sections` | optional |

A root Component without `boundary` is `private`; a nested one takes its parent's boundary. An
`external` Component is a system outside the product that it depends on.

| Constraint field | |
|---|---|
| `key`, `name` | required |
| `body`, `tags` | optional |
| `constraintBinding` | optional: the category it binds, below |
| `conservationKind` | optional: `sum_preservation`, `cap` or `operation_identity` |
| `verificationFate` | optional: `proved`, `monitored` or `sampled` |
| `mentions`, `ext`, `sections` | optional |

| `constraintBinding` field | |
|---|---|
| `category` | required: a name from `spec/categories.yaml` |
| `requiredValue` | required: the value the Constraint requires |

A Constraint sits under a Component, a Feature Group or a Product, and binds what it sits under:
the Reading List above holds one for the whole product, and its Shelf group one for the shelf.

```yaml path=spec/components/shelf-store.yaml
key: 00000000-0000-7000-8000-000000000001
name: "Shelf store"
body: "Where the shelf is kept, in the device's own storage."
```

## Quality Attributes

A Quality Attribute sits under a Feature or a Component.

| Quality Attribute field | |
|---|---|
| `key`, `name` | required |
| `body`, `tags` | optional |
| `qaKind` | required: `latency`, `throughput`, `availability`, `cost`, `testability`, `usability` or `capacity` |
| `qualityTarget` | required: below |
| `verificationFate` | optional: `proved`, `monitored` or `sampled` |
| `mentions`, `ext`, `sections` | optional |

| `qualityTarget` field | |
|---|---|
| `metric` | required |
| `band` | required: below |
| `unit` | required |
| `aggregation` | required: `each`, `single`, `minimum`, `maximum`, `mean`, `median`, `p95`, `p99` or `total` |
| `scenario`, `environment` | required |

| `band` field | |
|---|---|
| `kind` | required: `at_most`, `at_least`, `exactly` or `between` |
| `lower`, `upper`, `value` | optional: the bounds its `kind` takes |

`band.kind` decides its bounds: `at_most` takes `upper`, `at_least` takes `lower`, `exactly`
takes `value`, `between` takes `lower` and `upper`. Bounds are quoted decimals.

## Sections and bodies

A body is Markdown: paragraphs, lists, code spans, fenced code blocks, emphasis and links. Any
other construct, such as raw HTML, a table or a block quote, is reported as `V2-009`. A Section
holds prose that needs its own heading. Any node with a body may hold Sections. A heading inside
a body belongs in a Section instead (`V2-001`).

| Section field | |
|---|---|
| `key`, `name`, `body` | required |
| `mentions`, `ext`, `sections` | optional: `sections` nests Sections |

## Extension data

`ext` holds what a tool keeps on a node that the format does not define, such as the name an
OpenSpec requirement had, each tool under its own namespace. No lint rule reads it, it is not
part of a node's content hash, and the lock pins it. Leave it as the tool wrote it, except when
you copy a node: a copied Feature keeps its `openspec.capability`, and two Features with one
capability make `loft render --openspec` refuse, so remove it from the copy.

```yaml fragment
ext:
  openspec:
    name: "Sign in with email"
    metadata:
      - label: "ID"
        value: "AUTH-1"
```

A namespace is a lowercase letter followed by lowercase letters and digits, and each key of a
mapping is a letter followed by letters and digits. A value is text in double quotes or a literal
block, an integer, `true` or `false`, a list or a mapping; neither a list nor a mapping may be
empty.

## Tags and categories

Both registries are lists of entries.

| Tag field | | Category field | |
|---|---|---|---|
| `key` | required | `key` | required |
| `name` | required: lowercase letters, digits and hyphens | `name` | required: such segments joined by dots |
| `definition` | required | `definition` | required |

```yaml path=spec/tags.yaml
- name: offline
  definition: "Behaviour that works without a network connection."
```

```yaml path=spec/categories.yaml
- name: storage.limit
  definition: "A limit on how much the product stores."
```

## Decisions

| Decision field | |
|---|---|
| `key`, `slug`, `title`, `context`, `state` | required |
| `drivers`, `options`, `assessments`, `consequences`, `subjects`, `mentions` | optional |

| `state` field | |
|---|---|
| `kind` | required: `proposed`, `accepted`, `rejected`, `superseded` or `retired` |
| `selectedOption`, `outcome`, `rationale`, `successor` | optional: as its `kind` decides, below |

`state.kind` decides the other state fields:

| `kind` | Also holds |
|---|---|
| `proposed` | nothing |
| `accepted` | `selectedOption`, `outcome` |
| `rejected` | `rationale` |
| `superseded` | `selectedOption`, `outcome`, `successor` (a decision) |
| `retired` | `selectedOption`, `outcome`, `rationale` |

| Driver field | | Option field | |
|---|---|---|---|
| `key` | required | `key`, `title`, `description` | required |
| `kind` | required: `local` or `subject` | | |
| `label`, `statement` | optional: a `local` driver holds both | | |
| `subject` | optional: a `subject` driver holds it, a node | | |

| Assessment field | | Consequence field | |
|---|---|---|---|
| `option` | required: an option of this decision | `key` | required |
| `driver` | required: a driver of this decision | `kind` | required: `benefit`, `cost`, `risk` or `obligation` |
| `rating` | required: `supports`, `neutral`, `conflicts` or `unknown` | `statement` | required |
| `rationale` | required | `subject` | optional: a node |

A retired or superseded decision that names a node the document no longer holds names it by its
former label, the label it last carried, which `loft fmt` writes in place of its key.

| Subject field | | Former label field | |
|---|---|---|---|
| `role` | required: `affected` or `outcome` | `formerLabel` | required: the node's last label |
| `node` | required: a node | | |

```yaml path=spec/decisions/keep-the-shelf-on-the-device.yaml
title: "Keep the shelf on the device"
context: "Readers add books on the move, often without a connection."
state:
  kind: accepted
  selectedOption:
    name: "On the device"
  outcome: "The shelf is kept in the device's own storage."
drivers:
  - kind: local
    label: "Offline"
    statement: "A book can be added without a connection."
options:
  - title: "On the device"
    description: "The shelf lives in the device's storage."
  - title: "On a server"
    description: "The shelf lives on a server and is fetched when shown."
assessments:
  - option:
      name: "On the device"
    driver:
      name: "Offline"
    rating: supports
    rationale: "the device holds the shelf without a connection"
  - option:
      name: "On a server"
    driver:
      name: "Offline"
    rating: conflicts
    rationale: "adding a book needs the server"
consequences:
  - kind: cost
    statement: "A shelf is not shared between a reader's devices."
subjects:
  - role: affected
    node:
      name: "Shelf store"
  - role: outcome
    node:
      name: "When the reader adds a book by its title, the shelf shall hold the book."
```

The decision above is written in one pass: its options, drivers and subjects are named, and
`loft fmt` writes their keys, as the next section says.

## Edges

| Field | From | To |
|---|---|---|
| `realizes` | a Feature | the Components that implement it |
| `scopedTo` | a Component | the Products it serves |
| `subjects` | a decision | the nodes it affects or that state its outcome |
| `mentions` | any node or decision | any node or decision its prose refers to |

Every edge carries its target's key, as does every reference of a decision. While you write, a
reference may instead name its target, as a record of one field, `name`; `loft fmt` replaces it
with the key of the one element of the kind the field takes that has that name:

| Reference | Looks for |
|---|---|
| `realizes` | a Component, by its name |
| `scopedTo` | a Product, by its name |
| `mentions` | a node or a decision, by its name or title |
| a subject's `node`, a driver's or a consequence's `subject` | a node, by its name |
| `successor` | a decision, by its title |
| `selectedOption`, an assessment's `option` | an option of the same decision, by its title |
| an assessment's `driver` | a driver of the same decision, by its label or its subject's name |

A Requirement's name is its statement as one sentence. A name that matches nothing, or more than
one element, is an error at the reference, and nothing is formatted. To read a key, run
`loft find`.

## Lint configuration

`spec/lint.yaml` holds `rules` and `suppressions`. A rule entry names its `rule` and sets
`enabled`, `severity` (`warning`, `info` or `hint`) or the options the rule takes. A suppression
names a `rule`, the `key` of the node it spares and the `reason`.

```yaml path=spec/lint.yaml
rules:
  - rule: V2-008
    properNames:
      - "Reading List"
  - rule: LQ-005
    severity: hint
suppressions:
  - rule: SH-008
    key: 00000000-0000-7000-8000-000000000001
    reason: "The shelf's limit is a Constraint of the Shelf Feature Group."
```

The rules, their options and their defaults are in `lint-findings.md`.

## The trace file

`loft.trace.yaml`, beside `spec/`, links nodes to the tests that verify them. It is not part of
the specification: it enters no root, and no publish writes it. `reconciling.md` shows one and
says when to write it.

| Trace file field | | Test field | |
|---|---|---|---|
| `format` | required: `loft-trace/2` | `file` | required: its path from the directory that holds `spec/` |
| `links` | optional | `name` | required: the test's name |
| | | `body` | optional: the test's digest when it was last recorded; `loft reconcile --accept` writes it |

| Link field | |
|---|---|
| `node` | required: the key of an acceptance criterion, or of a Constraint or Quality Attribute whose `verificationFate` is `proved` |
| `reviewed` | optional: the node's content hash when the link was last accepted; `loft reconcile --accept` writes it |
| `tests` | optional: the tests that verify the node |
| `unverified` | optional: the reason no test verifies the node |

A link holds exactly one of `tests` and `unverified`, and no two links name one node. A file
whose `format` is `loft-trace/1` is still read; its tests have no `body`.
