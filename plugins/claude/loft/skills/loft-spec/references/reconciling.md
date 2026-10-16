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

The work is every `TR-001`, `TR-002`, `TR-003`, `TR-006` and `TR-009` finding on a node the change
touches: a node with no test yet, a link never reviewed, a link reviewed before its node changed,
a test changed after it was recorded, and a recorded test no longer located. Fix every `TR-004`,
`TR-005` and `TR-010` finding wherever it is: each says a link is no longer true, or that its test
cannot be read.

## Link a test to its node

Write the test first, then its link. Read the node's key with `loft find <label>` and add one
entry under `links`, or a test to the node's existing entry:

```yaml path=loft.trace.yaml
format: loft-trace/2
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
  its file writes it, a function name or a test's title. `loft reconcile` never runs it: in Rust,
  TypeScript, JavaScript, Python and Go it locates the test in the file's syntax tree, and
  elsewhere it finds the name as a whole word.
- Leave out a test's `body`: accepting writes it.
- Where no test can verify a node, write `unverified` with the reason instead of `tests`.
- Order does not matter; `loft reconcile --accept` writes the links in the document's order.

## Accept a link after review

Accepting a link records that a reviewer read its tests against the node as it now reads, and
records each located test's `body`, the digest of its tokens. Whitespace, comments, a trailing
comma and line endings do not count; any other change of a token does. Accepting also removes the
`body` of a test found only as a word. Read each test of the link against
every part of the node's words first; then run `loft reconcile --accept <label>`, once per node
or with `--accept` repeated. A link whose test cannot be found, or whose node is not in scope, is
refused, and so is a link whose test a part of its file that does not parse may cut short
(`TR-010`); nothing is written.

`loft reconcile --accept-stale` accepts every unreviewed and stale link, every link whose test
changed, and every link whose recorded test is no longer located, at once, and records the body of each located test that has none. It states a review of
each link it accepts, so run it only when the user asks for it.

## Fix each finding

| Rule | What it means | Fix |
|---|---|---|
| `TR-001` | No link names a node in scope | Write a test and link it, or link it `unverified` with the reason |
| `TR-002` | The link was never reviewed | Review its tests, then accept it |
| `TR-003` | The node changed after the link was reviewed | Read the node's new words; change the test where it no longer verifies them; accept it |
| `TR-004` | A test's file does not exist, lies outside the project, is not UTF-8 text, cannot be read, or does not hold its name | Correct the path or the name, or remove the test from the link; the message says which |
| `TR-005` | The link's node is gone, or is not in scope | Remove the link, or link the node that replaced it |
| `TR-006` | A test changed after its `body` was recorded | Read the test against the node; change it where it no longer verifies the node; accept the link |
| `TR-007` | A test is found only as a word, so a change to it goes unreported | Where its language is one `loft` parses, link the test by its function's name or its title; otherwise leave it |
| `TR-008` | A literal in backticks in the node's name occurs in none of its tests | Check that the link names the tests that verify the node; add a check of the literal where no test makes one |
| `TR-009` | A test whose `body` was recorded is now found only as a word, as when it was renamed and its old name left in a comment | Link the test by its present name; or, where it cannot be located any more, review it and accept the link, which removes its `body` |
| `TR-010` | A part of the test's file that the grammar does not parse lies in the test or after it, before the next test, so the test may be cut short | Make the file parse at the line and column the message gives; where the code is valid and only the grammar fails, report it and rewrite the line |

Never weaken a node's words to fit a test; change the test, or report the gap.

## Commit the trace file with the code

Commit `loft.trace.yaml` in the same commit as the tests it names. A CI job runs
`loft reconcile --fail-on warning`, which fails on every finding but `TR-001`, `TR-007` and
`TR-008`.
