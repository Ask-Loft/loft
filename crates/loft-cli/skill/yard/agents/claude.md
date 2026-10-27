## Claude Code adaptation

The Loft plugin records the questions you ask with `AskUserQuestion`, the plans you present
with `ExitPlanMode` and the tasks you create and update. Keep stages, notes, decisions and Loft
reports yourself using the tools below.

Task capture is attempted before the native task call returns, in the same order as your other
Yard captures. A new task uses the stage selected before its native creation runs. Waiting for
a question or plan decision leaves later captures free to run.

You reach it through six tools:

| Tool | What it does |
| --- | --- |
| `mcp__loft__yard_read` | Yard Log, or with `since` only what changed after that revision; each `loft` block's report by its size, or with `reports` the report itself. |
| `mcp__loft__yard_write` | A `batch` of blocks, applied whole or not at all, based on the revision in `basedOn`. With `send: true`, its questions, approvals and plans open a submission. |
| `mcp__loft__yard_wait` | Waits until the person has answered or decided a `submission`, and returns what they gave. |
| `mcp__loft__yard_withdraw` | Withdraws a `submission` that no longer needs answers. |
| `mcp__loft__yard_close` | Closes Yard Log once the branch's work is finished. |
| `mcp__loft__yard_published` | On the base branch, records the publish that followed a merge in the Yard Log of the `branch` the merge brought in. |

The tools run for your host, session and optional subagent. Do not supply another agent's
identity or write answers or outcomes on the person's behalf.

`yard_wait` holds the turn until the person answers on the page or in the dialog, or the
submission is withdrawn, or Yard Log is closed.

A question you ask with `AskUserQuestion` reaches Yard Log as well, in your active stage, and is
answered in either place in the same way. Its blocks take ids such as `ask-r12-1`; read Yard Log
to find them when a decision names the question.

The tasks you create with `TaskCreate` are mirrored as `task` blocks of your active stage, and
each `TaskUpdate` writes the new state: `in_progress` as `active`, `completed` as `done`.
Create a fix with `TaskCreate` and `metadata: { type: "fix" }`, or write its `task` block with
`type: fix`. A `TaskUpdate` with a `type` in its metadata changes it; `null` makes it `regular`.

## Plan the build in plan mode once the change is approved

Once the person has approved a change to `spec/` and it is committed, call `EnterPlanMode` before
you write any code of the build, even where the person asked for the build as well. In plan mode,
read the specification back and present the build as a plan with `ExitPlanMode`, worked out from
the specification the person approved: which criteria each step meets, and the tests that will
verify them. The build begins only once the person approves that plan; a plan they reject you
revise in plan mode with their reason, and present again.

A plan you present with `ExitPlanMode` reaches Yard Log as a `plan` block and may be approved or
rejected on the page or in the terminal. Approved on the page, it ends plan mode and you go on in
auto mode.

Where the person changes a requirement once the plan is approved, commit the change to `spec/` and
record its `spec-change` view before you write any code that meets it. Then call `EnterPlanMode`
and present with `ExitPlanMode` only the steps whose criteria the change touches, each as it now
stands. The steps of the approved plan that the change leaves untouched stand, and are not
presented again.

Record the publish with `yard_published` on the base branch once `loft publish` is committed.
Where `yard_read` reports that git ignores `.yard/`, commit nothing of `.yard/`.
When the branch's work is finished, close Yard Log with `yard_close`, commit the closed Yard Log
unless git ignores `.yard/`, and only then merge.
