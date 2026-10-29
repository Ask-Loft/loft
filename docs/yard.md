# Yard

Yard is where you and your coding agents meet while they work. Each branch has a Yard Log: what
the agents planned, asked, decided and did on it, as stages and the blocks in them. The Yard page
shows every Yard Log on your machine, live, and is where you answer questions, approve changes
and plans, and watch the work go.

You don't write Yard Log yourself. The `yard` skill, which ships with the Loft plugin, tells
the agent to open it when work on a branch begins and keep it as it goes. The plugin records
what the agent does through the host's own tools.

## What the agent keeps

- **Stages.** One for each step of the work, with a goal that says what the step leaves done.
  Stages still to come are written as planned, so you can see what's ahead.
- **Questions, approvals and plans.** The agent sends the questions of one decision point
  together and waits for your answers. In a repository with a Loft specification, it sends each
  change to `spec/` for approval before committing it, then presents a plan of the build before
  writing code.
- **Decisions.** Each choice you make, with what follows from it.
- **Tasks.** The bounded tasks of the plan, each marked active as it starts and done as it ends,
  so the page can time them.
- **Notes.** What each stage produced, with tables and Mermaid diagrams where they help.
- **Loft's reports.** The specification change, the findings of `loft check`, the trace and the
  publish state, recorded as `loft` printed them rather than retold.

## The page

The plugin keeps one page server running for the machine, on the loopback interface only, at
<http://127.0.0.1:7411/>. Every session on the machine shares it, whatever its repository or
worktree.

- **What waits on you** comes first, across every branch. Inside a branch's Yard Log it opens in
  a panel along the bottom, over a page that stays live, so you can read the log while you
  consider a plan or a question.
- **A branch's Yard Log** is drawn as a timeline of its stages, from the work done through the
  stage in hand to the stages still planned. Every earlier version of a block is kept and can be
  opened.
- **What changed since your last visit** is marked: what the agents wrote and what was answered
  while you were away.
- **A Gantt chart** of the sessions shows when each stage and task ran, including several agents
  working in one stage at once.
- **Tokens** each stage took, by agent and by model, where the host reports them.

## Answering

Answer in whichever place is in front of you. In Claude Code, the questions the agent asks with
`AskUserQuestion` and the plans it presents with `ExitPlanMode` show both in Claude Code's own
dialog and on the page; whichever you answer first counts, and the other closes. A plan approved
on the page ends plan mode, and the agent goes on in auto mode. A rejection with a reason sends
the reason back to the agent.

In Codex, questions and plans go to the page, and approving there records your decision. Codex's
own modes and permissions still govern what the agent may run.

## Hosts

### Claude Code

Install the plugin (`/plugin marketplace add Ask-Loft/loft`, then `/plugin install loft@ask-loft`)
and have `loft` on the path. The plugin and `loft` should be the same version: `loft init` and
`loft skill install` end with a report that says whether the `loft` the hooks find is, and how to
move an installed plugin to that version. The plugin's settings:

| Setting | Default | What it is |
| --- | --- | --- |
| `loftCommand` | `loft` | The command that runs `loft`, such as `npx --no-install loft` or `pnpm exec loft` |
| `yardPort` | `7411` | The port of the page server, shared by every session on the machine |
| `baseBranch` | `develop` | The base branch Yard Log records where git names none |

The plugin works the same on every machine. Where an organization manages Claude Code, or the
session is signed in with a Team or Enterprise account, Claude Code's built-in guard skips a
user's hooks on several events. So the plugin gives each session its Yard context, follows plan
mode and compactions, and takes the page's decision of a plan through command hooks that run
`loft yard claude hook`, which work under the guard.

After `/clear` or a resume, the new session continues the one that ended: it keeps that
session's stages, tasks and open questions, and starts from what it last read.

### Codex

Install the Codex plugin (`codex plugin marketplace add Ask-Loft/loft`, then
`codex plugin add loft@ask-loft`, or choose it in `/plugins`), then review and trust its hooks in
`/hooks`. Installing the skill alone doesn't activate them. Keep hooks enabled
(`features.hooks = true`). Codex may ask for another review after a hook definition changes.

The plugin's hooks run `loft yard codex hook`, and a background `loft yard codex watch` that
Codex owns. It sends a heartbeat every five seconds, restarts the page server if it stops and
keeps at most 1,000 queued events. A watch lasts at most 24 hours, and later activity starts
another. This works in local Codex sessions only, not with cloud orchestration.

The Codex adapter differs from Claude Code's:

- Codex's own dialogs are not synchronised with the page.
- A successful `update_plan` mirrors its steps as tasks of the agent's working stage.
- Token usage is unavailable, and tool activity whose subagent is ambiguous is left out.
- No transcript is parsed.

Each host's sessions are kept apart, so Claude Code and Codex can share a branch without sharing
an agent's state.

## Several agents on one stage

A task can be assigned to another agent while staying in its stage: a main agent delegates its
tasks to subagents, which work in its stage without owning it. The stage's owner or the task's
current assignee can reassign or release it, and a worker can claim an unassigned task for
itself. The agent binds its native task to the Yard task: with `metadata.yardTask` in Claude
Code, and with a `[yard:TASK-ID]` prefix on a Codex plan step.

An agent's end never marks its work done. Its active tasks return to open, still assigned, for
the stage's owner to review. The page shows each task's assignee apart from the stage's owner,
and each attempt at it as a bar of its own, without counting overlapping time twice.

## Where Yard Log lives

Yard Log is YAML under `.yard/<branch>/`, committed with the work, so a reviewer can read how the
branch was done. A repository that keeps its plans off git ignores `.yard/`. The agent closes
the branch's Yard Log before the merge. `loft merge-driver --install` makes git merge two copies
of a Yard Log record by record.

What belongs to the machine rather than the branch lives in `~/.yard/`: the page server's
address, the trees it serves, each session's events, the revision each agent last read, the
sessions that continue others and the control token.
