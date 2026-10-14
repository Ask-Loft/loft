# Reconciling the code

Use this workflow for keeping `loft.trace.yaml` true with `loft reconcile`: each node in scope
names the tests that verify it, and a change to the specification yields the list of tests to
write or review. It owns finding that work, linking a test to its node, accepting a link, and
what each `TR` finding means. The fields of `loft.trace.yaml` belong to `format.md`; the order of
the specification's commit and the code's to `changing-a-specified-product.md`; the files `loft`
writes for you to `the-loop.md`. Never restate or override them here.

## Contents

- Find the work
- Link a test to its node
- Accept a link after review
- Fix each finding
- Commit the trace file with the code

## Find the work

Run `loft reconcile` from the directory that holds `spec/`. The nodes in scope are every
acceptance criterion, and each Constraint and Quality Attribute whose `verificationFate` is
`proved`. Each line names a finding's node by its file, line, label and message, and the last
line counts the nodes in scope, the nodes a link names, and the findings of each rule.

The work is every `TR-001`, `TR-002` and `TR-003` finding on a node the change touches: a node
with no test yet, a link never reviewed, and a link reviewed before its node changed. Fix every
`TR-004` and `TR-005` finding wherever it is: each says a link is no longer true.

## Link a test to its node

Write the test first, then its link. Read the node's key with `loft find <label>` and add one
entry under `links`, or a test to the node's existing entry:

```yaml path=loft.trace.yaml
format: loft-trace/1
links:
  - node: 01a13900-0000-7000-8000-000000000001
    tests:
      - file: "tests/shelf.rs"
        name: "a_book_added_with_a_title_appears_on_the_shelf"
  - node: 01a13900-0000-7000-8000-000000000002
    unverified: "Checked by hand before each release: no test can see the shop's printer."
```

- `node` is the node's key, never its label: a publish may give the node another label.
- `file` is the test's path from the directory that holds `spec/`; `name` is the test's name as
  its file writes it, a function name or a test's title. `loft reconcile` finds it as a whole
  word in the file and never runs it.
- Where no test can verify a node, write `unverified` with the reason instead of `tests`.
- Order does not matter; `loft reconcile --accept` writes the links in the document's order.

## Accept a link after review

Accepting a link records that a reviewer read its tests against the node as it now reads. Read
each test of the link against every part of the node's words first; then run
`loft reconcile --accept <label>`, once per node or with `--accept` repeated. A link whose test
cannot be found, or whose node is not in scope, is refused, and nothing is written.

`loft reconcile --accept-stale` accepts every unreviewed and stale link at once. It states a
review of each, so run it only when the user asks for it.

## Fix each finding

| Rule | What it means | Fix |
|---|---|---|
| `TR-001` | No link names a node in scope | Write a test and link it, or link it `unverified` with the reason |
| `TR-002` | The link was never reviewed | Review its tests, then accept it |
| `TR-003` | The node changed after the link was reviewed | Read the node's new words; change the test where it no longer verifies them; accept it |
| `TR-004` | A test's file does not exist, lies outside the project, or does not hold its name | Correct the path or the name, or remove the test from the link |
| `TR-005` | The link's node is gone, or is not in scope | Remove the link, or link the node that replaced it |

Never weaken a node's words to fit a test; change the test, or report the gap.

## Commit the trace file with the code

Commit `loft.trace.yaml` in the same commit as the tests it names. A CI job runs
`loft reconcile --fail-on warning`, which fails on every finding but `TR-001`.
