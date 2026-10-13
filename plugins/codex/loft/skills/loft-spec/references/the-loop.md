# The loop

Use this workflow for every edit of `spec/`: writing nodes, formatting them, checking them and
committing them. It owns the order of the steps, the YAML that the reader accepts, and the files
and fields that `loft` writes for you. What each record holds belongs to `format.md`, how to word
it to `writing-rules.md`, and what each finding means to `lint-findings.md`. Never restate or
override them here.

## Contents

- Write, format, check, commit
- Reference a node by its key
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
   should not carry. Return to step 2.
5. Commit `spec/` once `loft fmt --check` and `loft check --fail-on warning` pass.

`loft fmt` changes only layout and identity, never your words. Run it as often as you like.

## Reference a node by its key

A reference names its target by the target's key: a Feature's `realizes`, a Component's
`scopedTo`, every `mentions`, a decision's `subjects`, and a decision's own `selectedOption`,
`option` and `driver`. A key exists only once `loft fmt` has minted it, so:

1. Write the target node first.
2. Run `loft fmt`, then read the target's new `key` from its file.
3. Write the reference with that key.

Never invent a key to reference a node that does not exist yet.

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
  Labels live only in `spec/labels.yaml`, which `loft publish` writes. Prose may cite a label;
  `writing-rules.md` says how.
- Never edit `spec/labels.yaml` or `loft.lock.json`.
- Never copy a key from one node to another, or from an example into a document.
