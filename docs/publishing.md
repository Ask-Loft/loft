# Publishing and merging

Ask your agent to publish when the specification reads right; the `loft-spec` skill knows the
steps below and when they apply.

## Publishing

`loft publish`, run on the publish branch (`main` unless `spec/loftspec.yaml` names another) over
a committed tree, allocates labels to new nodes in `spec/labels.yaml` and writes
`loft.lock.json`, which pins the specification's state, content and binding roots. Commit both
together. `loft verify` recomputes those roots and compares them with the lock, so CI can check
that `main` carries only published states. A second `loft publish` with nothing changed writes
nothing.

Until a publish, nodes have keys but no labels (`RQ-12`, `AC-40`). Work on other branches is a
candidate; the published state, the one the lock pins, is the contract.

## The merge driver

`loft merge-driver --install`, run once in each clone, registers Loft as git's merge driver for
`spec/` and `.yard/`: it adds `spec/**/*.yaml merge=loft` and `.yard/**/*.yaml merge=loft` to
`.gitattributes` and sets `merge.loft.driver` in the clone's configuration. Commit
`.gitattributes`, so that every clone routes both to the driver. git then merges each file of the tree by key,
and each Yard Log record by record. Where two branches change one field apart, the file gains a
conflict record in place of conflict markers, git reports the path as conflicted, and
`loft check` reports the record until it is removed.

git runs the driver through the shell, from the top of the working tree, with its own search
path, which a dev dependency is not on. Give the command that starts `loft` there with
`--command`: `loft merge-driver --install --command "npx --no-install loft"`, `"pnpm exec loft"`,
`"uv run loft"`, or a path such as `node_modules/.bin/loft`. The install runs
`<command> --version` as git would and refuses when it fails. The setting lives in the clone's
`.git/config`, so a project runs the install in its setup step, such as npm's `prepare` script.
If the command cannot run during a merge, the driver merges the file as text with conflict
markers and leaves it conflicted; make the command run, then `git checkout -m <file>` merges it
again.

## Two publishes that meet

Two clones that publish apart can give one label to two nodes. When one pulls the other, by
`git pull` (a merge) or `git pull --rebase`, the merge driver keeps the disputed bindings out of
`spec/labels.yaml` in one conflict record and git stops. `loft publish`, run there on the
publish branch, settles them: the pulled side keeps its labels, the other side's colliding
labels are withdrawn and their nodes get fresh ones, citations of a withdrawn label are
rewritten in every element that mentions its node, and the lock is written anew. It then prints
the git command that finishes, `git add -A && git commit --no-edit` after a merge or
`git add -A && git rebase --continue` after a rebase; Loft itself never changes the repository.
