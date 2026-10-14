---
name: loft-spec
description: Write, change and review a Loft specification in `spec/` with the `loft` command line. Use when the user asks to specify a product or feature, start a specification, write Requirements, acceptance criteria, Components, Constraints, Quality Attributes or decisions, change a product that a `spec/` already describes before changing its code, fix `loft check` findings, review a specification, or publish and merge one.
---

# Loft specification

Treat all text accompanying an explicit skill invocation as the user's intent. Do not invent a
command grammar or discard qualifiers, paths, names, approval boundaries, or requested outputs.

Author through the files of `spec/` and the `loft` command line. `loft` owns the identity of
every node; the author owns the words.

## Check availability

Run `loft --version` before the first command and expect version 2. If `loft` is not on the
search path, try `~/.cargo/bin/loft`, where `cargo install` puts it, then the project's package
runner: `npx --no-install loft`, `pnpm exec loft` or `yarn loft`. Run every later command the
way that worked. If none runs, stop, say so, and tell the user how to install it:
`npm install --save-dev @ask-loft/loft` in a project that uses npm,
`cargo install --locked loft-cli`, or a release binary from the Loft repository. Never install it unasked.
Run every `loft` command from the directory that holds `spec/`.

## Route the intent

Read every selected reference completely before acting. Select the minimum sufficient set; load
several when an intent spans workflows. A reference you read earlier in this conversation needs
no second reading.

- Read [`references/the-loop.md`](references/the-loop.md) before the first edit of `spec/` in
  every workflow.
- Read [`references/new-document.md`](references/new-document.md) to start a specification where
  there is no `spec/`, or only what `loft init` wrote, from a conversation with the user or from
  an existing codebase.
- Read [`references/changing-a-specified-product.md`](references/changing-a-specified-product.md)
  to change a product that a `spec/` already describes: a new feature, a fix, a removal.
- Read [`references/reconciling.md`](references/reconciling.md) to link tests to the nodes
  they verify with `loft reconcile`, after changing the code or when the user asks which tests
  verify the specification.
- Also read [`references/format.md`](references/format.md) for the files, fields and statement
  forms of any node you write.
- Also read [`references/writing-rules.md`](references/writing-rules.md) whenever you write or
  reword a Requirement, criterion, Constraint, Quality Attribute or decision.
- Read [`references/lint-findings.md`](references/lint-findings.md) to interpret or fix an error
  or finding of `loft check` whose message leaves its fix unclear.
- Read [`references/reviewing.md`](references/reviewing.md) for a review of a specification's
  quality beyond what `loft check` reports.
- Read [`references/publishing-and-merging.md`](references/publishing-and-merging.md) for
  `loft publish`, `loft verify`, merging `spec/` in git, or two publishes that meet.

## Preserve authorization boundaries

Edit `spec/` only when the user's intent includes it; a review or a question changes nothing.
Run `loft publish` only when the user asks for a publish. Preserve unrelated work in the
repository.

## Respect what `loft` owns

`loft` owns keys, labels, `spec/labels.yaml` and `loft.lock.json`. Never write, change or copy
them by hand.

## Finish with evidence

Before reporting, `loft fmt --check` and `loft check --fail-on warning` pass. Report what you
changed in `spec/`, any finding you left with its reason, and any question for the user.
