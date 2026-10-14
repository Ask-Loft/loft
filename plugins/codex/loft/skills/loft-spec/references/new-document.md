# A new document

Use this workflow for a specification started where there is no `spec/`, or only what
`loft init` wrote: drafted from a conversation with the user, or from an existing codebase that
has none. It owns the order of a
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
where `spec/` already exists.

The specification lives in git. If the directory is not in a git repository, ask the user before
running `git init`; never create a repository unasked. Once the project is under git, run
`loft merge-driver --install`, so that git merges `spec/` by key, and commit the `.gitattributes`
it writes, as `publishing-and-merging.md` says. The last line of `loft init` says what the merge
driver still needs: the install, or putting the project under git first.

## Draft from a conversation

1. Ask the user about the product: who uses it, what they can do with it, the limits it keeps,
   the systems it depends on, and what is out of scope. Ask a few questions at a time.
2. Keep asking until every answer that would change the outline or a Requirement is in hand:
   each kind of user, each thing they can do, what happens when it fails or is refused, the
   data it keeps and for how long, and the edges of scope. Two questions are rarely enough. A
   question the user leaves open stays an open question.
3. Propose an outline: the Products, their Feature Groups and Features, and the Components that
   implement them. Name each Feature for what a user can do.
4. Agree the outline with the user before writing any criterion. A Feature moved or merged now
   costs one line; later it costs every criterion beneath it.
5. Write the outline as files, then each Feature's Requirements and criteria, one Feature at a
   time, running the loop after each.
6. Record each choice the user made with alternatives in view as a decision.

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
left with its reason. Open questions go in the report, never into `spec/`: not as a node, a
body or a Section. Never call the draft ready for implementation while a question that would
change it is open. Publishing comes later, at the user's request.
