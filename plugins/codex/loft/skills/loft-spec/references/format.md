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
- Products and Feature Groups
- Features and Requirements
- Statements
- Components and Constraints
- Quality Attributes
- Sections and bodies
- Tags and categories
- Decisions
- Edges
- Lint configuration
- The trace file

## The files of `spec/`

| Path | Holds | Written by |
|---|---|---|
| `spec/loftspec.yaml` | The manifest: format, algorithm, document key, Products in order | `loft init`, `loft fmt` |
| `spec/labels.yaml` | Label counters and the bindings of labels to keys | `loft publish`; `loft fmt` drops a deleted node's binding |
| `spec/tags.yaml` | The tag registry | you |
| `spec/categories.yaml` | The category registry for Constraint bindings | you |
| `spec/lint.yaml` | The lint configuration, optional | you |
| `spec/products/<product>/product.yaml` | A Product and its Feature Groups | you |
| `spec/products/<product>/<group>/<feature>.yaml` | One Feature, in its group's directory | you |
| `spec/components/<component>.yaml` | One root Component and everything beneath it | you |
| `spec/decisions/<decision>.yaml` | One decision | you |
| `loft.lock.json`, beside `spec/` | The lock that pins the published tree | `loft publish` only |
| `loft.trace.yaml`, beside `spec/` | The links from nodes to the tests that verify them | you; `loft reconcile --accept` writes `reviewed` |

A file's name is its slug: lowercase letters, digits and hyphens. No other path under `spec/` is
allowed, but files and directories whose names start with `.` are ignored. A registry with no
entries is an empty file.

Fields are written in the order the tables below give; an optional field that is absent, or a
list that is empty, is left out. Required fields are marked; `key` and `slug` are required in a
formatted tree, and `loft fmt` writes them.

## Products and Feature Groups

| Product field | | Feature Group field | |
|---|---|---|---|
| `key` | required | `key` | required |
| `slug` | required | `slug` | required |
| `name` | required | `name` | required |
| `body` | optional | `body` | optional |
| `tags` | optional | `tags` | optional |
| `mentions` | optional | `mentions` | optional |
| `featureGroups` | optional | `features` | written by `loft fmt` |
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
| `realizes` | optional | `criteria` | optional | `sections` | optional |
| `mentions` | optional | `sections` | optional | | |
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
| `components` | optional: nested Components |
| `constraints`, `qualityAttributes`, `sections` | optional |

A root Component without `boundary` is `private`; a nested one takes its parent's boundary. An
`external` Component is a system outside the product that it depends on.

| Constraint field | |
|---|---|
| `key`, `name` | required |
| `body`, `tags` | optional |
| `constraintBinding` | optional: `category` (a name from `spec/categories.yaml`) and `requiredValue` |
| `conservationKind` | optional: `sum_preservation`, `cap` or `operation_identity` |
| `verificationFate` | optional: `proved`, `monitored` or `sampled` |
| `mentions`, `sections` | optional |

A Constraint sits under a Component, a Feature Group or a Product, and binds what it sits under:
the Reading List above holds one for the whole product, and its Shelf group one for the shelf.

```yaml path=spec/components/shelf-store.yaml
key: 00000000-0000-7000-8000-000000000001
name: "Shelf store"
body: "Where the shelf is kept, in the device's own storage."
```

## Quality Attributes

A Quality Attribute sits under a Feature or a Component: `key`, `name`, `body`, `tags`, `qaKind`,
`qualityTarget`, `verificationFate`, `mentions`, `sections`, with `name`, `qaKind` and
`qualityTarget` required.

- `qaKind`: `latency`, `throughput`, `availability`, `cost`, `testability`, `usability` or
  `capacity`.
- `qualityTarget`: `metric`, `band`, `unit`, `aggregation`, `scenario` and `environment`, all
  required.
- `band.kind` decides its bounds: `at_most` takes `upper`, `at_least` takes `lower`, `exactly`
  takes `value`, `between` takes `lower` and `upper`. Bounds are quoted decimals.
- `aggregation`: `each`, `single`, `minimum`, `maximum`, `mean`, `median`, `p95`, `p99` or
  `total`.

## Sections and bodies

A body is Markdown: paragraphs, lists, code, emphasis and links. A Section holds prose that needs
its own heading: `key`, `name`, `body` (required), `mentions` and nested `sections`. Any node with
a body may hold Sections. A heading inside a body belongs in a Section instead (`V2-001`).

## Tags and categories

Both registries are lists of `key`, `name` and `definition`. A tag name is lowercase letters,
digits and hyphens; a category name is such segments joined by dots.

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

`state.kind` decides the other state fields:

| `kind` | Also holds |
|---|---|
| `proposed` | nothing |
| `accepted` | `selectedOption`, `outcome` |
| `rejected` | `rationale` |
| `superseded` | `selectedOption`, `outcome`, `successor` (a decision) |
| `retired` | `selectedOption`, `outcome`, `rationale` |

- A driver is `key`, `kind: local`, `label` and `statement`, or `key`, `kind: subject` and
  `subject` (a node).
- An option is `key`, `title` and `description`.
- An assessment is `option`, `driver`, `rating` (`supports`, `neutral`, `conflicts` or
  `unknown`) and `rationale`.
- A consequence is `key`, `kind` (`benefit`, `cost`, `risk` or `obligation`), `statement` and an
  optional `subject`.
- A subject is `role` (`affected` or `outcome`) and `node`.

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
| `mentions` | any node | any node its prose refers to |

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

`loft.trace.yaml`, beside `spec/`, holds `format`, which is `loft-trace/1`, and `links`. It is
not part of the specification: it enters no root, and no publish writes it. `reconciling.md`
shows one and says when to write it.

| Field of a link | Holds |
|---|---|
| `node` | Required. The key of an acceptance criterion, or of a Constraint or Quality Attribute whose `verificationFate` is `proved` |
| `reviewed` | The node's content hash when the link was last accepted; `loft reconcile --accept` writes it |
| `tests` | The tests that verify the node, each with a required `file`, from the directory that holds `spec/`, and a required `name` |
| `unverified` | The reason no test verifies the node |

A link holds exactly one of `tests` and `unverified`, and no two links name one node.
