# The loop

Use this workflow for every edit of `spec/`: writing nodes, formatting them, checking them and
committing them. It owns the order of the steps, the YAML that the reader accepts, and the files
and fields that `loft` writes for you. What each record holds belongs to `format.md`, how to word
it to `writing-rules.md`, and what each finding means to `lint-findings.md`. Never restate or
override them here.

## Contents

- Write, format, check, commit
- Reference another node
- Expect `loft fmt` to reshape the file
- Write the YAML the reader accepts
- Leave identity to `loft`

## Write, format, check, commit

1. Write or edit the files of `spec/`. Write each new node without a `key`, and a new file
   without a `slug`.
2. Run `loft fmt`. It mints a key for every node that has none, sets each new file's slug from
   its file name, adds a new Feature file to its Feature Group and a new Product to the
   manifest, and rewrites every file in canonical layout.
3. Run `loft check --format json` and read its document. Its `errors` stop everything: fix each
   one first. Then read its `findings`, each with a `rule`, a `severity` and the place it names.
4. Fix each error and each finding at warning, then each finding below warning that the text
   should not carry. Read `lint-findings.md` only for a finding whose message leaves its fix
   unclear. Return to step 2.
5. Commit `spec/` once `loft fmt --check` and `loft check --fail-on warning` pass.

`loft fmt` changes only layout and identity, never your words. Run it as often as you like.

## Reference another node

A reference carries its target's key: a Feature's `realizes`, a Component's `scopedTo`, every
`mentions`, a decision's `subjects` and `successor`, its state's `selectedOption`, an
assessment's `option` and `driver`, and a driver's or consequence's `subject`. Write it by the
target's name instead, as a record of one field, and `loft fmt` writes the key in its place:

```yaml fragment
realizes:
  - name: "Shelf store"
```

`format.md` says what each field looks for by name. The target may be one you write in the same
edit: `loft fmt` mints its key first. A name that matches nothing, or more than one element, is
an error at the reference, and nothing is formatted; make the name exact.

To read the key or label of a node that exists, run `loft find` with its label, its slug or part
of its name: it prints each match's key, label, kind, file and name. Never invent a key.

## Expect `loft fmt` to reshape the file

`loft fmt` writes each file in canonical layout: each item of a list of records gains a `key:`
line first, quotes that YAML does not need are dropped, fields move into the Format's order, and
each reference by name becomes a key. Read a file again after `loft fmt` before you edit it; an
edit prepared against the text you wrote may no longer match.

## Write the YAML the reader accepts

The reader takes a strict subset of YAML and refuses the rest with an error at its line:

- no comments, anchors, aliases, tags or document markers;
- no flow collections such as `[a, b]` or `{a: 1}`: write a list one item per line;
- no single-quoted or folded scalars: double-quote free text, and use a literal block (`|`) for
  a body of several paragraphs;
- no tabs in indentation, and no field written twice.

Double-quote every name, body, statement slot, title and other free text. Write each field in
the order `format.md` gives; `loft fmt` restores the order if you do not.

## Leave identity to `loft`

- Never write or change a `key`. A node keeps its key for life, through every rename and move.
- Never write a label such as `RQ-12` where a key belongs, and never give a node a label field.
  Labels live only in `spec/labels.yaml`, which `loft publish` writes and `loft fmt` clears of
  deleted nodes. Prose may cite a label;
  `writing-rules.md` says how.
- Never edit `spec/labels.yaml` or `loft.lock.json`.
- Never copy a key from one node to another, or from an example into a document.
