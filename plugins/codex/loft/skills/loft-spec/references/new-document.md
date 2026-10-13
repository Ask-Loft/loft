# A new document

Use this workflow for a specification started where there is no `spec/`: drafted from a
conversation with the user, or from an existing codebase that has none. It owns the order of a
new document's first draft: creating the tree, agreeing an outline, then writing. Changing a
product that a `spec/` already describes belongs to `changing-a-specified-product.md`; the edit
cycle to `the-loop.md`; the wording to `writing-rules.md`. Never restate or override them here.

## Contents

- Create the tree
- Draft from a conversation
- Draft from an existing codebase
- Finish the first draft

## Create the tree

Run `loft init` in the directory that will hold `spec/`, usually the root of the product's
repository. It creates `spec/` with a manifest, label counters and empty registries, and refuses
where `spec/` already exists. If the repository is a git repository and more than one person
will edit `spec/`, also run `loft merge-driver --install`, as `publishing-and-merging.md` says.

## Draft from a conversation

1. Ask the user about the product: who uses it, what they can do with it, the limits it keeps,
   the systems it depends on, and what is out of scope. Ask a few questions at a time.
2. Propose an outline: the Products, their Feature Groups and Features, and the Components that
   implement them. Name each Feature for what a user can do.
3. Agree the outline with the user before writing any criterion. A Feature moved or merged now
   costs one line; later it costs every criterion beneath it.
4. Write the outline as files, then each Feature's Requirements and criteria, one Feature at a
   time, running the loop after each.
5. Record each choice the user made with alternatives in view as a decision.

## Draft from an existing codebase

1. Read the code before proposing anything: its entry points, its modules, its tests and its
   documentation. Tests and error messages state behaviour most precisely.
2. Propose an outline as above, the Components following the code's own parts.
3. List each behaviour you infer from the code as a question for the user: "The importer skips
   a row with no title; is that the rule?" Code shows what the product does, not what it should
   do, and a bug is not a Requirement.
4. Write a behaviour as a Requirement only once the user confirms it. Keep the rest as open
   questions in your report, not in `spec/`.

## Finish the first draft

Run the loop until `loft fmt --check` and `loft check --fail-on warning` pass, then commit
`spec/` on its own: "spec: first draft". Report the outline, the open questions, and any finding
left with its reason. Publishing comes later, at the user's request.
