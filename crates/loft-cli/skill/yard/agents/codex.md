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

The startup context is your briefing, from `loft yard read --context --mark-read`: its read is
recorded as your last, so a later read with `--since` starts from what it showed. A later prompt's
context says only whether decision notifications reach you. Read with `--mark-read` before your
first write, then base your write on that result's revision. Run the commands in the worktree
whose branch you are recording. Each result reports the page's address and whether git
ignores `.yard/`; where it is ignored, leave it out of commits.

### Questions and plan approval use the Yard page

Write the questions, approvals and plans of one decision point in a batch with `--send`.
Give the person the page address, then await the decision using automatic monitoring or explicit
waiting as described below. Read the answers and outcomes before proceeding. Withdraw a
submission the work has overtaken. Never treat a timeout as approval, and never write the
person's answers yourself.

Yard submissions use the page rather than native questions. Native question dialogs and native
plan approvals are not synchronized with the page. Page approval does not switch Codex out of
plan mode or grant permissions: Codex's mode and permissions still govern what you may execute.
When the host prevents a required write, explain the constraint and keep the plan in the
conversation until the host permits that write.

### Automatic decisions and explicit waiting

Session context reports whether decision notifications are available, unavailable or uncertain.
When decision notifications are available, you may end your turn after sending the submission.
The adapter's worker monitors fully resolved submissions and notifies your already loaded
native thread when a decision arrives; automatic monitoring fulfills the shared workflow's wait.
Do not keep the agent polling or keep a turn alive just to await the person.

When availability is unavailable, uncertain or not reported, wait with `--submission` set to the
returned submission key. A wait that returns `pending` after 540 seconds has no answer yet:
read what changed, then wait again while the decision is needed. If automatic monitoring becomes
unavailable while you are active and a decision is still needed, use this explicit waiting
fallback. Neither availability nor the end of a turn is an approval.

An adapter notification identifies the Yard Log, tree, branch, submission and resolution revision;
it is not a new approval. Before acting, read with `--mark-read` using your supplied caller identity
in the named tree and take the authoritative counted answers and outcomes from Yard. A retry
after uncertain delivery may produce an exceptional duplicate notification: make a fresh Yard read,
check what work is already done, and do not repeat completed work because another ping arrived.

Interruption or session end disarms automatic continuation. A later genuine user prompt can rearm
monitoring; an adapter-generated notification cannot. Normal turn completion leaves monitoring armed.
Native mode, permissions and hook trust continue to govern all work.

### Planning workflow

This workflow is adapted from [OpenAI Codex's Plan mode prompt](https://github.com/openai/codex/blob/rust-v0.164.0/codex-rs/collaboration-mode-templates/templates/plan.md)
at release `rust-v0.164.0`, Copyright 2025 OpenAI, licensed under Apache-2.0. Its questions,
plan presentation and completion boundary are adapted for Yard. No prompt in this skill changes
Codex's native collaboration mode.

Use this workflow when the person asks for a plan, and for the build plan required after
specification approval and its separate commit. A changed requirement needs a plan for only the
steps it affects. Keep implementation stages planned until the person has approved their plan
and native mode permits their build.

#### Planning boundary

Do not implement the change while planning. In particular, do not edit product files,
specifications or tests, apply patches, run migrations or code generation, run rewriting
formatters, or commit the implementation. Explore through reads, searches, static inspection
and dry runs. Focused tests or builds may write caches and build artifacts when they establish
facts needed for the plan and respect the person's testing limits.

Yard planning records are the only writes this workflow permits: planning stages and their
tasks, questions, approvals, plans, notes and decisions, plus the build's stages left `planned`.
Native mode and permissions still govern those writes. If native Plan mode prevents a Yard
write, keep the plan in the conversation and explain that the person must change the native
mode before it can be recorded. Never bypass that limit or claim that page approval changed
the mode.

`update_plan` is an execution-progress checklist: it does not enter or exit native Plan mode,
present a reviewable build plan or approve it. Do not use it to start implementation tasks while
planning. Once the build is approved and permitted, use the task-mirroring workflow below.

#### 1. Explore the repository

Explore before asking about discoverable facts. Read the relevant specification, implementation,
entry points, configuration, schemas and tests. Check the existing behavior and constraints with
targeted non-mutating commands. Resolve facts the repository can supply rather than asking the
person to locate code or choose something the existing implementation already establishes.
Ask before exploration only when the request itself has an obvious contradiction or ambiguity
that exploration cannot settle.

#### 2. Clarify intent

Establish the goal and success criteria, who the change serves, its scope, constraints and
material tradeoffs. Separate discoverable facts from the person's preferences. Resolve the
high-impact uncertainties before choosing an implementation approach.

Required questions go through Yard submissions: group one decision point's questions, give
meaningful options with a recommendation and its reasoning, send them together and wait for the
answers as described above. Ask only questions whose answers affect the plan or settle an
important assumption. Do not replace an unanswered required decision with an assumed default.
Record low-impact defaults that need no question as assumptions in the plan.

#### 3. Resolve implementation

Work out the interfaces and data flow, failure cases, compatibility and migrations, verification
and acceptance criteria, and any rollout needs relevant to the change. Tie the implementation
steps and tests to the approved specification's criteria where one exists. Ground the approach
in the repository and make the material implementation choices explicit. Continue exploration
or send another decision point's questions when a required choice is still open.

#### Present the plan and wait

Present one complete `plan` block with `title` and `markdown` once no material decisions remain
for the implementer. Keep the plan concise but complete: summary, implementation steps,
interfaces, tests and assumptions. Group steps by behavior or subsystem rather than listing
every file. Include risks and compatibility or rollout work where they affect implementation.

Submit the plan with `--send`, give the person the Yard page address, then await and read the
outcome using automatic monitoring or explicit waiting above. Leave the build's stages `planned` and wait for approval
before implementation. No question answer, timeout or native plan rendering counts as Yard plan
approval. An approved Yard plan authorizes only its described build when native mode and
permissions allow it.

Revise a rejected plan as a complete replacement, addressing the person's reason, and send it
in a new submission for approval. Withdraw any superseded submission that is still pending.
Do not present an incomplete replacement as the final plan.

Do not put `<proposed_plan>` tags in the Yard plan's Markdown. Where actual native Plan mode
requires that presentation in the conversation, follow the host's instruction there; that
presentation does not synchronize a native approval with the Yard page.

### Plan steps and reporting limits

The adapter mirrors successful `update_plan` calls into your working stage, the one your briefing
names: the stage of your active assigned tasks, else your own active stage, else your session's
main agent's active stage. Open your active stage before using `update_plan` where you have none
of these. It maps pending steps to open tasks, in_progress to active, and completed to done.
It identifies steps by exact text and duplicate occurrence: reordering retains their identities;
renaming creates a new task and drops the old unfinished task. Removed completed tasks stay done.
Do not duplicate these tasks in explicit writes.

When Codex does not offer `update_plan`, write Yard task blocks yourself. For a fix, read the
mirrored task and write its whole block with `type: fix`; later mirrors retain that classification.
Unbound steps need a working stage: none is mirrored while your active assigned tasks lie in more
than one stage. A step mirrored into a stage you do not own is assigned to you. Keep stages,
notes, decisions and Loft reports yourself.

For delegated work, claim or assign the existing task through a normal Yard write first. Use
its exact ID as the prefix of every bound native step: `[yard:TASK-ID] Local checklist wording`.
The adapter updates that task in its original stage without creating another task or requiring
a worker-owned stage. The prefix binds identity; the wording does not rename the task. Retain
the prefix on every update. Unknown, malformed or duplicate bindings create no replacement;
removing a bound step leaves the task unchanged. Another assignee's progress and terminal
delegated tasks are preserved.

An identified end or interruption returns your still-active assignments to open, retaining their
assignee and execution history. It never implies completion. A new start permits progress again;
late native updates from an ended worker are ignored. Ambiguous actor identity changes no task.
Startup context lists your assigned task IDs and original stages separately from your own stage;
read Yard Log for any assignments omitted from that bounded context.

Native hooks carry no token counters. During an identified native turn, the worker observes
latest-request usage from an already running local Codex app server over its Unix-domain
WebSocket. It uses `$CODEX_HOME/app-server-control/app-server-control.sock`, or the same path
under `~/.codex`; `loft yard codex watch --app-server-socket ABSOLUTE_PATH` selects another
local socket. It starts no Codex process and changes no configuration, collaboration mode or
permissions. The prompt hook waits at most 500 milliseconds for readiness; an absent server,
an unready new thread or an observation gap leaves usage unavailable or partial. It disconnects
when the turn ends, and reconnecting imports no historical token totals.

Fresh input excludes cached input, cached input is cacheRead, and output already includes
reasoning output. An absent cache-write counter stays unavailable. Zero cache writes from ChatGPT or unclassified
sources are unavailable. Verified API-key zeros and positive reported counts remain measured.
Raw source values are preserved in machine events and accounting checkpoints. Historical Codex
zeros without verified provenance are unavailable on the page; their source files are unchanged.
Native activity and
heartbeats continue without the usage source, and an unreported tool outcome stays unknown.
During concurrent subagent work, activity without a reliable agent identity is omitted.
The adapter does not parse transcripts. Its heartbeat worker has a 24-hour lifetime;
later hooked activity restarts a worker that ended. The integration supports local Codex
workflows; cloud orchestration does not run these plugin hooks.
