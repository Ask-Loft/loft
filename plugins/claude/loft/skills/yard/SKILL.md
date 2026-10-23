---
name: yard
description: Keep the branch's Yard Log while planning and building a change, ask the person's questions and approvals through it, and record what each step produced and decided. Use whenever work on a git branch is planned, staged, asked about or reviewed, when a session starts on a branch whose Yard Log has open stages or submissions, and before a branch is merged.
---

# Yard Log

Yard Log is the branch's record of how its work was planned, asked about, decided and done. You
write it as typed blocks; the person reads it on the Yard page, and answers and approves there or
in Claude Code's own dialog, whichever comes first. `loft` keeps it as YAML under
`.yard/<branch>/`, and it is committed with the work.

The Loft plugin keeps much of Yard Log without your help: the questions you ask with
`AskUserQuestion`, the plans you present with `ExitPlanMode` and the tasks you create and update
reach it by themselves. This skill covers what only you can write: the stages, what each step
produced, the decisions, and the reports of Loft.

You reach it through six tools:

| Tool | What it does |
| --- | --- |
| `mcp__loft__yard_read` | Yard Log, or with `since` only what changed after that revision; each `loft` block's report by its size, or with `reports` the report itself. |
| `mcp__loft__yard_write` | A `batch` of blocks, applied whole or not at all, based on the revision in `basedOn`. With `send: true`, its questions, approvals and plans open a submission. |
| `mcp__loft__yard_wait` | Waits until the person has answered or decided a `submission`, and returns what they gave. |
| `mcp__loft__yard_withdraw` | Withdraws a `submission` that no longer needs answers. |
| `mcp__loft__yard_close` | Closes Yard Log once the branch's work is finished. |
| `mcp__loft__yard_published` | On the base branch, records the publish that followed a merge in the Yard Log of the `branch` the merge brought in. |

Each result gives the revision and the page's address; give the person the address when they ask
where to answer.

## The blocks

Every block has an `id` (lowercase letters, digits and hyphens, starting with a letter) and a
`kind`. Every block but a stage names its `stage`. `after` places a block after another one; a new
block without it goes last. Writing a block again with the same id makes a new version of it,
which holds the whole block: write every field you mean to keep. Every earlier version stays in
Yard Log.

- `stage`: `title`, `goal` and `status` (`planned`, `active`, `done` or `dropped`).
- `note`: `markdown`, with an optional `title`, `tone` (`info`, `success`, `warning` or `danger`)
  and `folded`. Markdown only: a note holds no raw HTML.
- `question`: `prompt`, a short `header`, an optional `context` in Markdown, and two or more
  `options`, each with a `label`, a `detail` and `recommended`. `multiple` lets the person choose
  more than one option; `ownWords` lets them answer in their own words.
- `approval`: `title` and `markdown`: something the person approves, or rejects with a reason.
- `plan`: `title`, `markdown` and, for a plan file within the tree, its `path` relative to the tree.
- `decision`: `title`, `consequence`, and the `question` it settles, by the question's id.
- `task`: `title`, `state` (`open`, `active`, `done` or `dropped`) and `detail`.
- `loft`: a `view` of the branch's Loft specification (`spec-change`, `decision`, `trace`,
  `findings` or `publish`) and an optional `note`. `loft` runs the view and keeps its report; see
  below.

Only the person answers and decides: never write an answer or an outcome. Only `loft` records a
`loft` block's report: never write one, and never write a `loft` block again once written.

## Open Yard Log as the work begins

Your first write on a branch opens the branch's Yard Log, titled with the branch's name. Open it
when work on the branch begins, before the work's first step: write the stages you foresee, with
the first one `active`. Each branch has its own Yard Log, so cut the branch before you write.
Yard Log, once closed, takes no more writes: more work goes on a branch of its own.

## Read before you write

When a session starts, and each time a wait returns, read Yard Log with `since` set to the
revision you last read, and take in what the person answered or decided meanwhile. The plugin
gives you the revision you last read when the session starts. Base each write on the revision you
last read, in `basedOn`. A write refused because Yard Log changed names the blocks that changed
under you: read them, then write again on the new revision.

A read gives each `loft` block's report by its size in `reportBytes`, not the report: a report
can run to hundreds of kilobytes, and the page draws them for the person. It also gives the number
of items of each list it counts in `reportCounts`, such as 45 items in `graph.diff`: check what a report
holds by its counts, and read it whole only for what they do not say. Where you need what a report
holds, read with `reports` as well as `since`, so that only the reports you need come back.

## Open a stage for each step

Before each step of the work, write a stage for it, `active`, with a goal that names what the step
leaves done ("The store reads and writes Yard Log as YAML", not "Work on the store"). When the
step ends, mark the stage `done`, or `dropped` with a note on why, before you open the next. Write
the stages you foresee as `planned`, so the person sees the road ahead.

Each agent has its own active stage. A subagent you start writes to the same Yard Log as you when
it works in your worktree, and opens a stage of its own for its work rather than writing into
yours. A subagent that works in a worktree of its own, on a branch of its own, keeps that branch's
Yard Log.

## Plan, then wait to be asked to build

Begin no stage that builds the change unless the person asked for the build as well as the plan.
Where they asked you to plan, shape or specify the change, stop once the plan is written, and
leave the build's stages `planned`: they show the road ahead without starting down it. Tell the
person the plan is ready and that the build waits for them to ask for it. A plan the person
approved counts as asking for the build it describes, whether they approved it on the page, in
the terminal or in plan mode; a question they answered does not.

## Keep each task true as you go

Yard Log dates each task by the writes that change its state, so a task's state is only as true
as the moment it is written. The tasks you create with `TaskCreate` are mirrored as `task` blocks
of your active stage, and each `TaskUpdate` writes the new state: `in_progress` as `active`,
`completed` as `done`. You may also write `task` blocks yourself.

Either way, mark a task `active` in a write before its work starts. As soon as its work ends, mark
it `done`, or `dropped` with a reason in its `detail`, before the next task is marked `active`.
Never mark the tasks of a stage `done` in one write at its end: the ones not marked as they ran
show no time of their own on the page's Gantt chart.

## Ask once per decision point, then wait

Gather the questions of one decision point and send them together as one submission, rather than
ask them one at a time: write them with `send: true`, then call `yard_wait` with the submission
the write gave.

- Give each question a recommended option, and put the reasoning for each option in its `detail`:
  what choosing it leads to, and why you recommend the one you do.
- Use `context` for what the person needs to know to choose.
- Put an approval or a plan that belongs to the same decision point in the same submission.
- Keep option labels short: the questions are also asked in Claude Code's own dialog.

`yard_wait` holds the turn until the person answers on the page or in the dialog, or the
submission is withdrawn, or Yard Log is closed. Read the answers and outcomes from the wait's
result before you go on, and act on them; where the person rejected something, the reason says
what to change. Withdraw a submission that the work has overtaken rather than leave it waiting.

A question you ask with `AskUserQuestion` reaches Yard Log as well, in your active stage, and is
answered in either place in the same way. Its blocks take ids such as `ask-r12-1`; read Yard Log
to find them when a decision names the question.

## Write what each stage produced

When a stage ends, write a note of what it produced: what was built or found, what it means for
the next step, and anything left open. Write the note in Markdown, with a table where things are
compared and a `mermaid` diagram where a flow or a structure is clearer drawn than told. Use a
tone where it helps the reader: `success` for a result, `warning` for a risk.

## Record each choice as a decision

Each time the person makes a choice, by an answer or a decision, write a `decision` block: the
choice as its `title`, what follows from it as its `consequence`, and the question it settles as
its `question`. Decisions are numbered in the order they are written. The page draws a decision at
the head of the question it settles, so make its consequence say what follows from the choice
rather than repeat the answer.

## Record Loft's reports, don't retell them

Where the working tree holds a Loft specification (`spec/loftspec.yaml`), record a `loft` block at
each point of the specification's work, in place of a note that retells what `loft` printed.
`loft` runs the view when you write the block and keeps its report as it was then; a later state
is a later block.

- Record a `spec-change` view once a change to `spec/` is committed. It compares with the base
  branch, the branch this one was cut from, or with the revision you give as `base`.
- Record a `decision` view once a decision of the specification is accepted, naming it as
  `decision` by its slug, its key or its label.
- Record a `findings` view after `loft check`, with a reason for each finding left in place, in
  `reasons`: each with the finding's `rule`, the `key` of the node it names and the `reason`.
- Record a `trace` view once the tests are linked and the links accepted.
- Record the publish with `yard_published` on the base branch once `loft publish` is committed
  there. `loft` publishes only on its publish branch, so the publish comes after the branch is
  merged and its Yard Log closed: name the branch whose Yard Log the merge brought in, and commit
  `.yard/` after it. It is the one block that closed Yard Log takes.

Put what the report does not say in the block's `note`: why the change is shaped as it is, or what
the person should look at. A refused `loft` block names the command that failed and why: fix the
cause rather than writing the report as a note.

## Plan the build in plan mode once the spec is written

Where the working tree holds a Loft specification, the specification comes first and the code
after it. Once a change to `spec/` is committed and its `spec-change` view recorded, call
`EnterPlanMode` before you write any code of the build, even where the person asked for the build
as well. In plan mode, read the specification back and present the build as a plan with
`ExitPlanMode`, worked out from the specification you just wrote: which criteria each step meets,
and the tests that will verify them. The build begins only once the person approves that plan; a
plan they reject you revise in plan mode with their reason, and present again.

A plan you present with `ExitPlanMode` reaches Yard Log as a `plan` block and may be approved or
rejected on the page or in the terminal. Approved on the page, it ends plan mode and you go on in
auto mode.

## Commit `.yard/` with the work

Commit `.yard/` in each commit of the work, beside the code it describes. Answers the person gave
since the last commit are committed with the next, and so is what Yard Client keeps for each
session by itself: its token totals under `.yard/<branch>/tokens/` and its activity under
`.yard/<branch>/activity/`.

Where `yard_read` reports that git ignores `.yard/`, the repository keeps Yard Log on the machine
alone: commit nothing of `.yard/`, and never add it to a commit by force.

## Close it before the merge

When the branch's work is finished, close Yard Log with `yard_close`, commit the closed Yard Log
unless git ignores `.yard/`, and only then merge. Closing withdraws any submission still open.
