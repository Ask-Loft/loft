# Changing a specified product

Use this workflow for a feature, a fix or a removal in a product that a `spec/` already
describes: the specification changes first, in its own commit, and the code follows it. It owns
finding the nodes a change touches, the edits that keep identity, what a branch and a commit
carry, and the order of the commits.
Starting a document where there is none belongs to `new-document.md`; the edit cycle to
`the-loop.md`; the wording to `writing-rules.md`. Never restate or override them here.

## Contents

- Find the nodes the change touches
- Edit the specification first
- Keep identity through edits
- Keep one intent to a branch and a commit
- Commit the specification, then the code
- Check the code against the criteria

## Find the nodes the change touches

- Run `loft find` with a label the user names, such as `RQ-12`, a slug, or part of a node's
  name: it prints each match's key, label, kind, file and name. `spec/labels.yaml` binds each
  label to its key.
- To find a node by its words, run `loft render --markdown <path>` with a path outside the tree,
  which writes the whole document as Markdown, with every node under its label, and search it. A
  plain `loft render` writes `docs/spec.md` and `docs/spec.json` beside `spec/`. The projection is
  for reading; never edit it, and never commit it unless the project does.
- Read the Feature that holds the behaviour, its Requirements and criteria, the Components it
  realizes, the decisions whose subjects include it, and every node that `mentions` it.
- Open the file `loft find` names and confirm the node's name and kind there before you edit it.
- Before deleting a Product or a tag, find every reference to it by name: a tag is listed on each
  Feature it marks, and a Product's Components are realized from elsewhere.

## Edit the specification first

- A new behaviour is a new Requirement with its criteria, or a new Feature; write it without a
  key.
- A changed behaviour is the same Requirement or criterion with new words. Change the statement
  or the criterion in place.
- A removed behaviour is a removed node. Remove its file, or its entry in its parent's list, then
  run `loft fmt`, which marks its binding departed in `spec/labels.yaml`. Remove every other
  reference to its key that `loft check` reports.
- A behaviour that contradicts a decision needs a new decision that supersedes the old one; ask
  the user before making it.

Agree a change of more than one Feature with the user before writing criteria, as for a new
document.

## Keep identity through edits

- A node keeps its key when its words change, and when it moves between Features or files.
- Never give one node's key to another, even to a node that replaces it: a replacement is a new
  node.
- A deleted node's label is never allocated again; a later publish gives new nodes new labels.
- A retired or superseded decision that names a deleted node names it by its former label, as
  a mapping whose single field `formerLabel` holds the label. `loft fmt` writes it when it marks
  the node's binding departed; a live decision that names a deleted node is an error to resolve
  with the user.

## Keep one intent to a branch and a commit

A branch carries one intent, and each of its commits one coherent step of it. Before committing,
read `git diff` against the intent, node by node, with `loft status --base <base>`, which lists
the nodes the branch added, changed and removed: every node it changes is one the intent
touches, and every node the intent touches is changed as intended. A clean `loft check` shows the
tree is well formed, not that it means what was intended.

## Commit the specification, then the code

Run the loop until `loft fmt --check` and `loft check --fail-on warning` pass, then commit `spec/`
alone, before any code, once the person has approved it where the Yard skill is in use: "spec:
<the change>". Then write the code against the committed
specification, in its own commits.

## Check the code against the criteria

Before reporting, walk each new or changed criterion and name the test or code that meets it.
Report any criterion the code does not yet meet; never weaken a criterion to fit the code. Then
reconcile the code as `reconciling.md` describes, so that `loft.trace.yaml` links each new test
to its node.
