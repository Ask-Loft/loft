# Adopting an OpenSpec project

Use this workflow for a `spec/` that `loft import openspec` wrote: taking it to one that
`loft check --fail-on warning` accepts, and keeping an OpenSpec projection of it while a team
switches. It owns reading the import's findings, what to keep of what the import recorded, and
the steps of the adoption in their order. The fields of `ext` and the rule for copying a Feature
belong to `format.md`; how to word a statement, a criterion, a Constraint or a decision to
`writing-rules.md`; linking tests to `reconciling.md`. Never restate or override them here. A
`spec/` that `loft import v1` wrote from a Loft v1 export takes none of these steps: run the loop
of `the-loop.md` over it.

## Contents

- Read the import report
- Keep what the import recorded
- Give each Feature its Components
- Rewrite the criteria
- Write the missing Purposes
- Lift Decisions and Constraints
- Link the tests
- Keep an OpenSpec projection
- Finish the adoption

## Read the import report

Read the findings `loft import openspec` printed before the first edit of `spec/`; where they are
not in the conversation, ask the user for them. Each names an `OS` rule and the OpenSpec file and
line it concerns. These ask for work:

| Rule | What the import did | What to do |
|---|---|---|
| `OS-002` | The Purpose was a placeholder, and the Feature has no body | Write the missing Purposes |
| `OS-003`, `OS-004`, `OS-005`, `OS-006` | Content outside OpenSpec's grammar became a Section | Ask the user whether the Section states requirements; write each one it states as a Requirement |
| `OS-007` | The requirement held neither SHALL nor MUST, and its statement is text | Write the statement in an EARS form |
| `OS-010` | Two requirements of one capability had one name | Ask the user whether one replaces the other |
| `OS-011` | Active changes were not converted | Carry each one out as a change to the specification, as `changing-a-specified-product.md` says, with its proposal as the brief |
| `OS-013`, `OS-014` | `config.yaml` holds a `context`, or names a store | Read the `context` for conventions; where a store holds the main specs, tell the user the import may be stale |

The other rules only report what the import skipped or changed.

The note after the findings counts the `V2-007` warnings `loft check` will report: OpenSpec states
a scenario's obligation in its steps, which became the criterion's body. Add the entry the note
gives to `spec/lint.yaml`, so that the warnings that remain are the ones to fix first:

```yaml path=spec/lint.yaml
rules:
  - rule: V2-007
    severity: info
```

## Keep what the import recorded

Leave each node's `ext.openspec` and the file `openspec-sidecar.json` beside `spec/` as the import
wrote them. `ext.openspec` holds the requirement names, capability paths and metadata lines that
`loft render --openspec` writes back; the sidecar records where each node came from. Read both
when the user asks where a node came from.

## Give each Feature its Components

The import creates no Component, so `SH-001` fires on every Feature. With the user, name the
parts of the system that implement the Features, write each as a Component, and list in each
Feature's `realizes` the Components that implement it.

## Rewrite the criteria

Each scenario became a criterion: its title became the criterion's name, and its steps its body.
A title such as "Valid credentials" names a situation, not an outcome. For each criterion:

- Rewrite the name as one checkable sentence that states the outcome the steps lead to.
- Keep the WHEN and THEN steps as the body.
- Move an obligation the body states, such as "THEN the system MUST lock the account", into the
  name; the step then says what happens, without SHALL or MUST.

Work one Feature at a time, and confirm a rewritten name with the user where the steps leave the
outcome open.

## Write the missing Purposes

Where the Purpose was a placeholder (`OS-002`), the Feature has no body. Write it with the user:
what the capability is for, in a sentence or two. `loft render --openspec` writes the body as the
spec's Purpose, in place of the placeholder.

## Lift Decisions and Constraints

The import read the main specs only. The rest of the project still holds what Loft can state:

- The `design.md` of each change under `openspec/changes/archive/`, or under `openspec/archive/`
  in an older project, records decisions and their
  reasons. Propose a Decision for each one the user confirms still holds, with state `proposed`.
- `project.md`, which became the Product's Section, and the `context` of `openspec/config.yaml`
  hold conventions. Propose a Constraint for each one that limits how the system is built.

## Link the tests

Link the tests that verify the rewritten criteria as `reconciling.md` says.

## Keep an OpenSpec projection

While the team still reads OpenSpec's files or runs its tools, write them from `spec/` rather than
by hand:

```
loft render --openspec openspec
```

- The render refuses a directory that already holds main specs, such as the one the import
  read. Give `--force` to replace them: a spec that no Feature renders to any more is removed, and
  each removed file is printed.
- Each finding is an `OR` rule. One whose message names OpenSpec says what
  `openspec validate --strict` will reject in the rendered files. Fix the node it names in
  `spec/`, never the rendered file.
- `loft-render-map.json`, in the directory rendered to, records the node each rendered requirement
  and scenario comes from, with its file and line.

## Finish the adoption

The import writes `spec/` and its sidecar only. Set up what `loft init` would have: run
`loft skill install` and `loft merge-driver --install`, then commit `spec/`, the sidecar, the
skill directories and agent settings `loft skill install` names, and `.gitattributes` together.

Once no criterion's body states an obligation, remove the `V2-007` entry from `spec/lint.yaml`,
and fix each `V2-007` warning that returns. Report to the user what remains: Features without a
Purpose, criteria whose outcome only the user can state, and decisions left unproposed.
