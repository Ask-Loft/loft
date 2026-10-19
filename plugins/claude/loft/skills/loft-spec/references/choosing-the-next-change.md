# Choosing the next change

Use this workflow for finding what to build next from a specification's published states, and for
planning it, before any code is written. It owns which published change is chosen, how it is
built by key, the scenarios planned for it and the report. Changing the specification belongs to
`changing-a-specified-product.md`; linking tests to their nodes to `reconciling.md`. Never restate
or override them here.

Choosing reads and changes nothing: it edits no file, commits nothing and publishes nothing.
Planning, and then building, begin only when the user asks for them.

## Contents

- Read the pinned state
- Choose the change
- Build the change by key
- Plan the scenarios
- Report

## Read the pinned state

Read `loft.lock.json` first: its publish sequence, publication time and state root name the state
the code is built to. `loft status` reports it and whether the tree beside it matches.

The published states are contracts, not a queue of tasks: a state says what the product must do,
not what is left to do. Uncommitted edits of `spec/`, and edits on a branch not yet merged and
published, are candidates, not the contract; report them apart, and never build them as the next
change unless the user says so.

## Choose the change

Use the first of these that applies:

1. **A state the user names**, by its publish sequence or its commit. Build to it; if it is the
   pinned state, look for what of it the code does not meet yet.
2. **The earliest published change the code does not yet meet.** On the publish branch,
   `git log --format='%h %s' -- loft.lock.json` lists the publishes, newest first. Take them in
   order from the pinned one, and skip none.

Never choose a state because it is the latest. Judge what the code meets from its tests, from
`loft.trace.yaml` and `loft reconcile`, and from the code itself; a node that no link names yet is
a question to answer, not proof that the code fails it.

## Build the change by key

What a publish changed is the difference between it and the publish before it, node by node, by
key. With the publish checked out, `loft status --base <the previous publish's commit>` lists the
nodes it added, changed and removed, each with its key, label and file; `loft review --base` gives
the same assertions with the nodes they are read against, moved ones included. A state is a whole
tree, not a patch: read each changed node in full, with its Feature, its criteria and the
Components it realizes.

Tell apart what the publish changed, what the code already does, and debt the code carried before
it. Only the first is the change.

## Plan the scenarios

A criterion is rarely met by one case alone. Plan the scenarios that the change's criteria
together call for, and map each scenario to every criterion it covers. Choose them by risk rather
than by every combination:

- **Identity:** a node created, renamed, removed, created again and reverted; a key that must
  survive each step.
- **Absence:** absent, null and empty values, where the specification tells them apart.
- **Return:** a value changed and changed back, `A → B → A`, where history keeps both.

Where the criteria do not decide what a scenario should give, report the ambiguity and ask; never
invent the behaviour.

## Report

Report, in this order:

1. the pinned state, from `loft.lock.json`;
2. the change chosen, and why that one;
3. the nodes it adds, changes and removes, by label, or by key where there is none yet;
4. the code and tests read to judge what is met;
5. the proposed work: the failing tests to write first, each mapped to its criteria, and the
   commits;
6. the questions and ambiguities that keep the scope open;
7. the next action: review, plan, reconcile or build.

When the user asked only what to build next, stop after the report.
