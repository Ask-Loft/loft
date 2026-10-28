// Task changes the mod makes for an agent, each one `loft yard` command that Yard Log applies to
// the task as it stands under its lock: no task is read first, and no change is retried.
import type { EngineInterface } from 'claude-code'

import { callerArgs } from './run'
import { actorKey, callerOf, run, state } from './state'

/**
 * Returns the agent's active tasks to open in one write, as `loft yard task --release` does, and
 * keeps its later native updates from being mirrored until it starts again.
 */
export async function endAssigned($: EngineInterface, agentId: string | undefined) {
  if (!state.isOn) return
  const caller = await callerOf($, agentId)
  state.endedTasks.add(actorKey(caller.session, agentId))
  await run($, await $.session.cwd(), ['task', '--release', ...callerArgs(caller)])
}
