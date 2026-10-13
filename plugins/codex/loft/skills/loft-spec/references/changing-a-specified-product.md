# Changing a specified product

Use this workflow for a feature, a fix or a removal in a product that a `spec/` already
describes: the specification changes first, in its own commit, and the code follows it. It owns
finding the nodes a change touches, the edits that keep identity, and the order of the commits.
Starting a document where there is none belongs to `new-document.md`; the edit cycle to
`the-loop.md`; the wording to `writing-rules.md`. Never restate or override them here.

## Contents

- Find the nodes the change touches
- Edit the specification first
- Keep identity through edits
- Commit the specification, then the code
- Check the code against the criteria

## Find the nodes the change touches

- Run `loft find` with a label the user names, such as `RQ-12`, a slug, or part of a node's
  name: it prints each match's key, label, kind, file and name. `spec/labels.yaml` binds each
  label to its key.
- To find a node by its words, run `loft render`, which writes the whole document as Markdown to
  `docs/spec.md`, with every node under its label, and search it. The projection is for reading;
  never edit it, and never commit it unless the project does.
- Read the Feature that holds the behaviour, its Requirements and criteria, the Components it
  realizes, the decisions whose subjects include it, and every node that `mentions` it.

## Edit the specification first

- A new behaviour is a new Requirement with its criteria, or a new Feature; write it without a
  key.
- A changed behaviour is the same Requirement or criterion with new words. Change the statement
  or the criterion in place.
- A removed behaviour is a removed node. Remove its file, or its entry in its parent's list, and
  every reference to its key.
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
  a mapping whose single field `formerLabel` holds the label.

## Commit the specification, then the code

Run the loop until `loft fmt --check` and `loft check --fail-on warning` pass, then commit `spec/`
alone, before any code: "spec: <the change>". Then write the code against the committed
specification, in its own commits.

## Check the code against the criteria

Before reporting, walk each new or changed criterion and name the test or code that meets it.
Report any criterion the code does not yet meet; never weaken a criterion to fit the code.
