---
name: yard
description: Keep the branch's Yard Log while planning and building a change, ask the person's questions and approvals through it, and record what each step produced and decided. Use whenever work on a git branch is planned, staged, asked about or reviewed, when a session starts on a branch whose Yard Log has open stages or submissions, and before a branch is merged.
---

# Yard Log

Yard Log is the branch's record of how its work was planned, asked about, decided and done. You
write it as typed blocks; the person reads it on the Yard page and answers and approves through
the page or a supported dialog. `loft` keeps it as YAML under `.yard/<branch>/`, and it is
committed with the work.

The host adaptation below names the operations you can call, the native actions recorded for
you, and how to present questions and plans. This shared workflow covers the stages, what each
step produced, the decisions and the reports of Loft. Use the adaptation's operations to read,
write, wait, withdraw, close and record a publish. Each result gives the revision and the page's
address; give the person the address when they ask where to answer.

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
- `task`: `title`, `state` (`open`, `active`, `done` or `dropped`), `detail` and `type`
  (`regular`, the default, or `fix`).
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
revision you last read, and take in what the person answered or decided meanwhile. The adapter
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
approved counts as asking for the build it describes, through any supported approval interface.
A question they answered does not, and a change to the specification the person approved does not
count as asking for its build: it settles what is built, not when.

## Keep each task true as you go

Yard Log dates each task by the writes that change its state, so a task's state is only as true
as the moment it is written. Write `task` blocks yourself, or use the task mirroring described
by the host adaptation where it is available.

Either way, mark a task `active` in a write before its work starts. As soon as its work ends, mark
it `done`, or `dropped` with a reason in its `detail`, before the next task is marked `active`.
Never mark the tasks of a stage `done` in one write at its end: the ones not marked as they ran
show no time of their own on the page's Gantt chart.

## Mark the fixes the person asked for

A task is a fix where the person asked you to correct work already done: a fault they found, a
change they reviewed, a plan or approval they rejected with a reason. A task you take on by
yourself, a correction of your own included, is `regular`. The page marks each fix apart, so the
person sees how much of a stage went on what they asked to have put right.

To record a fix, write its `task` block with `type: fix`, or use the host adaptation's task
metadata.

## Ask once per decision point, then wait

Gather the questions of one decision point and send them together as one submission, rather than
ask them one at a time: write them with `send: true`, then wait on the submission
the write gave.

- Give each question a recommended option, and put the reasoning for each option in its `detail`:
  what choosing it leads to, and why you recommend the one you do.
- Use `context` for what the person needs to know to choose.
- Put an approval or a plan that belongs to the same decision point in the same submission.
- Keep option labels short so they fit the supported dialogs.

The wait operation holds the turn until the person answers on the page or in the dialog, or the
submission is withdrawn, or Yard Log is closed. Read the answers and outcomes from the wait's
result before you go on, and act on them; where the person rejected something, the reason says
what to change. Withdraw a submission that the work has overtaken rather than leave it waiting.

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

- Record a `spec-change` view once a change to `spec/` passes `loft fmt --check` and
  `loft check --fail-on warning`, whether or not it is committed: it reads the working tree. It
  compares with the base branch, the branch this one was cut from, or with the revision you give
  as `base`.
- Record a `decision` view once a decision of the specification is accepted, naming it as
  `decision` by its slug, its key or its label.
- Record a `findings` view after `loft check`, with a reason for each finding left in place, in
  `reasons`: each with the finding's `rule`, the `key` of the node it names and the `reason`.
- Record a `trace` view once the tests are linked and the links accepted.
- Record the publish with the published operation on the base branch once `loft publish` is committed
  there. `loft` publishes only on its publish branch, so the publish comes after the branch is
  merged and its Yard Log closed: name the branch whose Yard Log the merge brought in, and commit
  `.yard/` after it. It is the one block that closed Yard Log takes.

Put what the report does not say in the block's `note`: why the change is shaped as it is, or what
the person should look at. A refused `loft` block names the command that failed and why: fix the
cause rather than writing the report as a note.

## Have the spec change approved before you commit it

Where the working tree holds a Loft specification, the specification comes first and the code
after it, and the person settles what the change says before anything is planned on it. Once a
change to `spec/` passes the checks, record its `spec-change` view, then send an `approval` of the
change right after its `spec-change` block, in the same stage: its Markdown says what changed and
what the person should look at, and the block above draws the change itself.

Where the person rejects the change, the reason says what to change: revise `spec/` without
committing it, then record a new `spec-change` block and send a new approval right after it. Each
round is a new pair, so each approval stands beside the change it asks about. Commit `spec/` once
the person approves the change, and not before, alone and as one commit.

## Plan the build once the change is approved

Once the person has approved a change to `spec/` and it is committed, plan the build before you
write any code of it, even where the person asked for the build as well. Read the specification
back and present a plan worked out from what the person approved: which criteria each step
meets, and the tests that will verify them. Use the host adaptation's planning interface. The
build begins only once the person approves that plan; a plan they reject you revise with their
reason and present again.

Where the person changes a requirement once the plan is approved, commit the change to `spec/` and
record its `spec-change` view before you write any code that meets it. Plan again only the steps
whose criteria the change touches, each as it now stands. The steps of the approved plan that the
change leaves untouched stand, and are not presented again.

## Commit `.yard/` with the work

Commit `.yard/` in each commit of the work, beside the code it describes. Answers the person gave
since the last commit are committed with the next, and so is what Yard Client keeps for each
session by itself: its token totals under `.yard/<branch>/tokens/` and its activity under
`.yard/<branch>/activity/`.

Where a read reports that git ignores `.yard/`, the repository keeps Yard Log on the machine
alone: commit nothing of `.yard/`, and never add it to a commit by force.

## Close it before the merge

When the branch's work is finished, close Yard Log with the close operation, commit the closed
Yard Log unless git ignores `.yard/`, and only then merge. Closing withdraws any submission still
open.

## Codex adaptation

Use the `loft` command line for Yard. Native hooks supply session context, record supported
activity and keep the local page server running. The plugin and a `loft` with the Codex adapter
must be installed, and the plugin's hooks enabled and trusted in Codex.

### Caller and operations

Use the identity supplied by the session hook: `--host codex --session SESSION`, plus
`--agent AGENT --agent-type TYPE` for a subagent. Keep that identity when working in another
worktree. Do not supply another caller's identity. Use `--format json` for every operation.

| Operation | Command |
| --- | --- |
| Read | `loft yard read --host codex --session SESSION --mark-read --format json` |
| Write | `loft yard write --host codex --session SESSION --input - --based-on REVISION --format json` |
| Wait | `loft yard wait --host codex --session SESSION --submission KEY --timeout 540 --format json` |
| Withdraw | `loft yard withdraw --host codex --session SESSION --submission KEY --format json` |
| Close | `loft yard close --host codex --session SESSION --format json` |
| Record a publish | `loft yard published --host codex --session SESSION --branch BRANCH --format json` |

Add your subagent arguments to each command where applicable. Pass a write's JSON batch on stdin
or through an input file; use argument arrays or correct shell quoting for values. A write's
`--based-on` is the revision you last read. On a stale refusal, read again and rebuild the write
from the new revision. Read with `--since REVISION` for changes since your last read, and add
`--reports` only when you need a Loft report in full.

The startup context is an adapter peek; it does not mark a read for you. Read with `--mark-read`
before your first write, then base your write on that result's revision. Run the commands in the
worktree whose branch you are recording. Each result reports the page's address and whether git
ignores `.yard/`; where it is ignored, leave it out of commits.

### Questions and plan approval use the Yard page

Write the questions, approvals and plans of one decision point in a batch with `--send`.
Give the person the page address, then wait with `--submission` set to the returned submission
key. Read the answers and outcomes before proceeding. A wait that returns `pending` after 540
seconds has no answer yet: read what changed, then wait again while the decision is needed.
Withdraw a submission the work has overtaken. Never treat a timeout as approval, and never write
the person's answers yourself.

Yard submissions use the page rather than native questions. Native question dialogs and native
plan approvals are not synchronized with the page. Page approval does not switch Codex out of
plan mode or grant permissions: Codex's mode and permissions still govern what you may execute.
When the host prevents a required write, explain the constraint and keep the plan in the
conversation until the host permits that write.

After an approved specification change, present the build as a Yard plan and wait for its page
approval before implementing. A changed requirement needs a plan for only the steps it affects.
Keep implementation stages planned until the person has asked for their build and the native
mode permits it.

### Plan steps and reporting limits

Open your active stage before using `update_plan`. The adapter mirrors successful calls into
that stage, mapping pending steps to open tasks, in_progress to active, and completed to done.
It identifies steps by exact text and duplicate occurrence: reordering retains their identities;
renaming creates a new task and drops the old unfinished task. Removed completed tasks stay done.
Do not duplicate these tasks in explicit writes.

When Codex does not offer `update_plan`, write Yard task blocks yourself. For a fix, read the
mirrored task and write its whole block with `type: fix`; later mirrors retain that classification.
No task is mirrored without an active stage belonging to its caller. Keep stages, notes,
decisions and Loft reports yourself.

Token usage is unavailable from this adapter's native hooks, and an unreported tool outcome
stays unknown. During concurrent subagent work, activity without a reliable agent identity is
omitted. The adapter does not parse transcripts. Its heartbeat worker has a 24-hour lifetime;
later hooked activity restarts a worker that ended. The integration supports local Codex
workflows; cloud orchestration does not run these plugin hooks.
