# Publishing and merging

Use this workflow for `loft publish`, `loft verify`, merging `spec/` in git, and two publishes
that meet. It owns the publish branch, the lock and the merge driver. The edit cycle belongs to
`the-loop.md`; what an error means to `lint-findings.md`. Never restate or override them here.

## Contents

- Publish
- Verify in CI
- Merge with the merge driver
- Settle a conflict record
- Settle two publishes that meet

## Publish

Run `loft publish` only when the user asks for it. It runs on the publish branch, `main` unless
`spec/loftspec.yaml` names another as `publishBranch`, over a `spec/` with no uncommitted
change. It gives each new node a label in `spec/labels.yaml`, and writes `loft.lock.json`, which
pins the tree. Commit those two files together, and nothing else, as the publish commit. A second
`loft publish` with nothing changed writes nothing.

A publish refuses on another branch, over uncommitted changes, or over a tree with errors; read
the refusal and report it rather than working around it.

## Verify in CI

`loft verify` recomputes the tree's roots and compares them with the lock. CI runs it on the
publish branch, where every commit carries a published state. Other branches hold changes the
next publish makes normative, so they run `loft fmt --check` and
`loft check --fail-on warning` instead.

## Merge with the merge driver

Run `loft merge-driver --install` once in each clone. git then merges each file of `spec/` by
key: two branches that edit different nodes, or different fields of one node, merge cleanly.
When git runs the driver through something other than a `loft` on its search path, pass the
command that starts it: `loft merge-driver --install --command "npx --no-install loft"`. When
the install writes `.gitattributes`, commit it: every clone needs the attribute, while the driver
itself is set in each clone's own configuration.

## Settle a conflict record

Where two branches change one field apart, the merged file holds a conflict record in place of
conflict markers, naming the field and each side's value, and `loft check` reports it. Decide the
value with the user, write it into the node, remove the record, and run the loop. If git could
not run the driver and left text conflict markers, make the driver's command run, then run
`git checkout -m <file>` to merge the file again.

A conflict over `position` keeps a record that the branches moved apart within one list at its
place in the base, each side naming the key of the record it follows there, or `first`. A
conflict over `parent` keeps a record that the branches moved to different parents in one file
at its base parent, each side naming the key of the parent it chose; settle it by moving the
record. A conflict over `record` keeps a record that one branch deleted and the other changed or
moved, with the deleting side `null`. The driver merges each file alone, so a node moved to
another file in one branch and changed in the other is written in both files, the old copy with
a `record` conflict, and `loft check` reports its key carried twice; keep one copy.

## Settle two publishes that meet

Two clones that publish apart can give one label to two nodes. When one pulls the other and git
stops on `spec/labels.yaml`, run `loft publish` in the stopped merge or rebase of the publish
branch. It keeps the pulled side's labels, gives the other side's nodes new ones, rewrites the
citations of each withdrawn label, writes the lock, and prints the git command that finishes the
merge or rebase. Run that command.
