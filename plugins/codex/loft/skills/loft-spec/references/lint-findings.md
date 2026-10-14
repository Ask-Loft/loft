# Lint findings

Use this workflow for what `loft check` reports: each kind of error, and each
lint rule with its meaning, its usual fix and a wrong example. It owns the meaning of errors and
findings and when to suppress one. The craft that the rules point at belongs to
`writing-rules.md`, and the fields to `format.md`. Never restate or override them here.

`loft check --format json` prints one document. Its `errors` stop the check: no finding is
reported until the tree reads and validates. Each finding names its `rule`, its `severity`
(`warning`, `info` or `hint`), a `message`, the place it names, and where it has them a `term`
and a `bodyLine`. CI usually runs `loft check --fail-on warning`.

Fix the text a finding points at, never only its wording: a rule rephrased into silence still
breaks the craft it checks. Suppress a finding only when the rule does not apply to that node,
with the reason in `spec/lint.yaml`:

```yaml fragment
suppressions:
  - rule: SH-002
    key: 01a1342c-8f1e-7b3a-9d40-5c2e8a71f0b4
    reason: "The parser is realized by the import Feature of another product."
```

The wrong examples below are checked: each is written over the small document that follows and
raises its rule, and no other rule at warning.

## Contents

- Errors
- The small document
- Structural rules
- Language rules
- Rules of the text form
- Rules of the writing craft

## Errors

| `kind` | Means | Usual fix |
|---|---|---|
| `read` | A file breaks the YAML subset, holds an unknown or missing field, a bad value, or sits at a path the format does not allow | Fix the file at the line and column given; see `the-loop.md` for the subset and `format.md` for the fields |
| `validation` | The tree reads but breaks a rule across nodes: a reference to a key that is not there, a decision incomplete for its state, a tag listed twice, a conflict record left by a merge | Fix the reference or complete the node; a conflict record is settled as `publishing-and-merging.md` says |
| `lint-config` | `spec/lint.yaml` names a rule, severity or option that does not exist | Fix the entry at the line given |
| `store` | `spec/` cannot be read: missing, unreadable, or not a tree | Run `loft` from the directory that holds `spec/` |
| `refused` | The command refused to act, such as `loft init` over an existing `spec/` | Read the message; it names what to change |
| `integrity` | The tree and the lock disagree | Only `loft verify` and `loft publish` report it; see `publishing-and-merging.md` |

## The small document

```yaml path=spec/products/notes/product.yaml
name: "Notes"
body: "A place to keep short notes."
featureGroups:
  - name: "Writing"
    body: "Creating and changing notes."
```

```yaml path=spec/components/note-store.yaml
key: 00000000-0000-7000-8000-000000000101
name: "Note store"
body: "Where notes are kept."
constraints:
  - name: "The note store shall keep each note on the device."
    verificationFate: proved
```

```yaml path=spec/products/notes/writing/write-a-note.yaml
name: "Write a note"
body: "Saving a short note."
realizes:
  - 00000000-0000-7000-8000-000000000101
requirements:
  - statement:
      form: event_driven
      when: "the user saves a note"
      system: "the note store"
      shall: "keep the note."
    criteria:
      - name: "The note is listed after it is saved."
      - name: "An empty note is not kept."
```

## Structural rules

| Rule | Default | Reports | Usual fix |
|---|---|---|---|
| `SH-001` | warning | A Feature with no `realizes` | Add the Components that implement it |
| `SH-002` | info | A leaf Component no Feature realizes; an `external` one is exempt | Realize it from its Feature, or remove it |
| `SH-003` | warning | A Requirement with no criterion | Write its criteria |
| `SH-004` | warning | A Feature with no Requirement | Write its Requirements |
| `SH-005` | info | A Product with no Feature Group, or a Feature Group with no Feature | Fill it or remove it |
| `SH-006` | info | A tag in the registry that no node lists | List it where it applies, or remove it |
| `SH-007` | warning | Two siblings with the same name | Name each for what distinguishes it |
| `SH-008` | info | A root Component with no Constraint or Quality Attribute in its subtree | State the rules it keeps |
| `SH-009` | warning | A node lists a tag that the registry does not define | Define it in `spec/tags.yaml` |
| `SH-010` | hint | A Component named like a file path | Name the part, not the file |

```yaml rule=SH-001 path=spec/products/notes/writing/write-a-note.yaml
name: "Write a note"
body: "Saving a short note."
requirements:
  - statement:
      form: event_driven
      when: "the user saves a note"
      system: "the note store"
      shall: "keep the note."
    criteria:
      - name: "The note is listed after it is saved."
```

```yaml rule=SH-002 path=spec/components/note-index.yaml
name: "Note index"
body: "The index of notes by title."
```

```yaml rule=SH-003 path=spec/products/notes/writing/write-a-note.yaml
name: "Write a note"
body: "Saving a short note."
realizes:
  - 00000000-0000-7000-8000-000000000101
requirements:
  - statement:
      form: event_driven
      when: "the user saves a note"
      system: "the note store"
      shall: "keep the note."
```

```yaml rule=SH-004 path=spec/products/notes/writing/write-a-note.yaml
name: "Write a note"
body: "Saving a short note."
realizes:
  - 00000000-0000-7000-8000-000000000101
```

```yaml rule=SH-005 path=spec/products/notes/product.yaml
name: "Notes"
body: "A place to keep short notes."
featureGroups:
  - name: "Writing"
    body: "Creating and changing notes."
  - name: "Sharing"
    body: "Sending notes to other people."
```

```yaml rule=SH-006 path=spec/tags.yaml
- name: synced
  definition: "Behaviour that copies notes between devices."
```

```yaml rule=SH-007 path=spec/products/notes/writing/write-a-note.yaml
name: "Write a note"
body: "Saving a short note."
realizes:
  - 00000000-0000-7000-8000-000000000101
requirements:
  - statement:
      form: event_driven
      when: "the user saves a note"
      system: "the note store"
      shall: "keep the note."
    criteria:
      - name: "The note is listed after it is saved."
      - name: "The note is listed after it is saved."
```

```yaml rule=SH-008 path=spec/components/note-store.yaml
key: 00000000-0000-7000-8000-000000000101
name: "Note store"
body: "Where notes are kept."
```

```yaml rule=SH-009 path=spec/products/notes/writing/write-a-note.yaml
name: "Write a note"
body: "Saving a short note."
tags:
  - offline
realizes:
  - 00000000-0000-7000-8000-000000000101
requirements:
  - statement:
      form: event_driven
      when: "the user saves a note"
      system: "the note store"
      shall: "keep the note."
    criteria:
      - name: "The note is listed after it is saved."
```

```yaml rule=SH-010 path=spec/components/note-store.yaml
key: 00000000-0000-7000-8000-000000000101
name: "src/note_store.rs"
body: "Where notes are kept."
constraints:
  - name: "The note store shall keep each note on the device."
    verificationFate: proved
```

## Language rules

They read statements, criteria, Constraints and Quality Attributes, and the bodies of those
nodes; text in code spans and code blocks is not prose. `LQ-001`..`LQ-004` take a `lexicon` that
`spec/lint.yaml` can `replace`, or `extend` and `remove`.

| Rule | Default | Reports | Usual fix |
|---|---|---|---|
| `LQ-001` | warning | A weak term such as "fast", "robust", "user friendly" | State the measurable outcome |
| `LQ-002` | warning | A vague quantifier such as "some", "several", "many"; "at most 3" is a bound | State the number or the set |
| `LQ-003` | warning | An escape clause such as "if possible", "as required" | State when it applies, or drop it |
| `LQ-004` | info, off | An absolute such as "all", "always", "never" | Check the absolute is meant |
| `LQ-005` | info | A text with two modal verbs or three conjunctions (`modalLexicon`, `conjunctionLexicon`, `conjunctionThreshold`) | Split it into one obligation each |
| `LQ-006` | warning | A Quality Attribute with no target; it never fires, since the target is required | — |
| `LQ-007` | info, off | A statement stored as text that matches none of the five forms | Write it as slots |
| `LQ-008` | info | Two names of one kind nearly alike (`minTokens`, `threshold`) | Name each for what distinguishes it |
| `LQ-009` | hint | Prose that uses a tag's name where neither the node nor a node above it lists the tag; a one-word tag matches its word, a hyphenated one only its hyphenated form | List the tag, or reword |

```yaml rule=LQ-001 path=spec/products/notes/writing/write-a-note.yaml
name: "Write a note"
body: "Saving a short note."
realizes:
  - 00000000-0000-7000-8000-000000000101
requirements:
  - statement:
      form: event_driven
      when: "the user saves a note"
      system: "the note store"
      shall: "keep the note."
    criteria:
      - name: "The note is saved fast."
```

```yaml rule=LQ-002 path=spec/products/notes/writing/write-a-note.yaml
name: "Write a note"
body: "Saving a short note."
realizes:
  - 00000000-0000-7000-8000-000000000101
requirements:
  - statement:
      form: event_driven
      when: "the user saves a note"
      system: "the note store"
      shall: "keep the note."
    criteria:
      - name: "Some notes are listed first."
```

```yaml rule=LQ-003 path=spec/products/notes/writing/write-a-note.yaml
name: "Write a note"
body: "Saving a short note."
realizes:
  - 00000000-0000-7000-8000-000000000101
requirements:
  - statement:
      form: event_driven
      when: "the user saves a note"
      system: "the note store"
      shall: "keep the note."
    criteria:
      - name: "The note is copied to the server if possible."
```

```yaml rule=LQ-004 path=spec/lint.yaml
rules:
  - rule: LQ-004
    enabled: true
```

```yaml rule=LQ-004 path=spec/products/notes/writing/write-a-note.yaml
name: "Write a note"
body: "Saving a short note."
realizes:
  - 00000000-0000-7000-8000-000000000101
requirements:
  - statement:
      form: event_driven
      when: "the user saves a note"
      system: "the note store"
      shall: "keep the note."
    criteria:
      - name: "The note is always listed."
```

```yaml rule=LQ-005 path=spec/products/notes/writing/write-a-note.yaml
name: "Write a note"
body: "Saving a short note."
realizes:
  - 00000000-0000-7000-8000-000000000101
requirements:
  - statement:
      form: event_driven
      when: "the user saves a note"
      system: "the note store"
      shall: "keep the note and list it and sort it and count it."
    criteria:
      - name: "The note is listed after it is saved."
```

```yaml rule=LQ-007 path=spec/lint.yaml
rules:
  - rule: LQ-007
    enabled: true
```

```yaml rule=LQ-007 path=spec/products/notes/writing/write-a-note.yaml
name: "Write a note"
body: "Saving a short note."
realizes:
  - 00000000-0000-7000-8000-000000000101
requirements:
  - statement:
      text: "Notes are kept."
    criteria:
      - name: "The note is listed after it is saved."
```

```yaml rule=LQ-008 path=spec/products/notes/product.yaml
name: "Notes"
body: "A place to keep short notes."
featureGroups:
  - name: "Writing"
    body: "Creating and changing notes."
  - name: "Sharing"
    body: "Sending notes to other people."
```

```yaml rule=LQ-008 path=spec/products/notes/sharing/write-a-note.yaml
name: "Write a note"
body: "Writing a note to another person."
realizes:
  - 00000000-0000-7000-8000-000000000101
requirements:
  - statement:
      form: event_driven
      when: "the user sends a note"
      system: "the note store"
      shall: "keep a copy of it."
    criteria:
      - name: "A copy of the sent note is listed."
```

```yaml rule=LQ-009 path=spec/tags.yaml
- name: offline
  definition: "Behaviour that works without a network connection."
```

```yaml rule=LQ-009 path=spec/products/notes/product.yaml
name: "Notes"
body: "A place to keep short notes, offline as well."
featureGroups:
  - name: "Writing"
    body: "Creating and changing notes."
```

## Rules of the text form

| Rule | Default | Reports | Usual fix |
|---|---|---|---|
| `V2-001` | info | A heading inside a body | Move the text under it into a Section |
| `V2-002` | info | A statement stored as `text` | Write it as `form` and slots |
| `V2-003` | warning | A label cited in prose without a `mentions` edge to its node, a label that names nothing, or a label withdrawn from a mentioned node, which the next publish rewrites | Add the `mentions` edge, or fix the label |
| `V2-004` | warning | A withdrawn label the next publish cannot rewrite, because the node mentions two candidates | Cite the node by its live label |
| `V2-009` | info | A Markdown construct in a body outside the subset: raw HTML, a block quote, a table, an image, a thematic break, an indented code block, a footnote, a task list item or struck-through text | Rewrite it as paragraphs, lists, code spans, fenced code blocks, emphasis and links |

`V2-003` and `V2-004` concern labels, so they arise only in a published tree.

```yaml rule=V2-001 path=spec/products/notes/writing/write-a-note.yaml
name: "Write a note"
body: |
  Saving a short note.

  # Why short

  Short notes are read.
realizes:
  - 00000000-0000-7000-8000-000000000101
requirements:
  - statement:
      form: event_driven
      when: "the user saves a note"
      system: "the note store"
      shall: "keep the note."
    criteria:
      - name: "The note is listed after it is saved."
```

```yaml rule=V2-002 path=spec/products/notes/writing/write-a-note.yaml
name: "Write a note"
body: "Saving a short note."
realizes:
  - 00000000-0000-7000-8000-000000000101
requirements:
  - statement:
      text: "When the user saves a note, the note store shall keep the note."
    criteria:
      - name: "The note is listed after it is saved."
```

```yaml rule=V2-009 path=spec/products/notes/writing/write-a-note.yaml
name: "Write a note"
body: |
  Saving a short note.

  > Short notes are read.
realizes:
  - 00000000-0000-7000-8000-000000000101
requirements:
  - statement:
      form: event_driven
      when: "the user saves a note"
      system: "the note store"
      shall: "keep the note."
    criteria:
      - name: "The note is listed after it is saved."
```

## Rules of the writing craft

| Rule | Default | Reports | Usual fix |
|---|---|---|---|
| `V2-005` | warning | A criterion name with a semicolon or more than one sentence | Split it into one criterion for each outcome |
| `V2-006` | info | A Requirement with more than `maxCriteria` criteria (20) or a body of more than `maxBodyWords` words (150) | Promote it to a Feature, as `writing-rules.md` says |
| `V2-007` | warning | A word of its `lexicon` ("shall", "must") in a body, outside code | Move the obligation into a statement or a criterion |
| `V2-008` | info | A statement whose system is "the" and a name in `properNames` | Drop the article |

```yaml rule=V2-005 path=spec/products/notes/writing/write-a-note.yaml
name: "Write a note"
body: "Saving a short note."
realizes:
  - 00000000-0000-7000-8000-000000000101
requirements:
  - statement:
      form: event_driven
      when: "the user saves a note"
      system: "the note store"
      shall: "keep the note."
    criteria:
      - name: "The note is listed; the count goes up."
```

The example sets `maxCriteria` low to keep it short.

```yaml rule=V2-006 path=spec/lint.yaml
rules:
  - rule: V2-006
    maxCriteria: 1
```

```yaml rule=V2-007 path=spec/products/notes/writing/write-a-note.yaml
name: "Write a note"
body: "Saving a short note. A note must never be lost."
realizes:
  - 00000000-0000-7000-8000-000000000101
requirements:
  - statement:
      form: event_driven
      when: "the user saves a note"
      system: "the note store"
      shall: "keep the note."
    criteria:
      - name: "The note is listed after it is saved."
```

```yaml rule=V2-008 path=spec/lint.yaml
rules:
  - rule: V2-008
    properNames:
      - "Notes"
```

```yaml rule=V2-008 path=spec/products/notes/writing/write-a-note.yaml
name: "Write a note"
body: "Saving a short note."
realizes:
  - 00000000-0000-7000-8000-000000000101
requirements:
  - statement:
      form: event_driven
      when: "the user saves a note"
      system: "the Notes"
      shall: "keep the note."
    criteria:
      - name: "The note is listed after it is saved."
```
