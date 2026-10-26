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
