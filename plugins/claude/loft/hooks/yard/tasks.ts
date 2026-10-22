// The agent's own tasks, mirrored as `task` blocks of the stage active for the agent that made them
// (the Feature "Mirror the agent's tasks").

import type { EngineInterface, ToolCallResult } from 'claude-code'

import { taskId, taskState } from './answers'
import { activeStage, blockById, whoOf } from './log'
import { callerArgs, succeeded, type Caller } from './run'
import { askerOf, callerOf, peek, run } from './state'

const failed = (ran: ToolCallResult) => ran.deny !== undefined || ran.isError === true

async function write($: EngineInterface, cwd: string, caller: Caller, block: Record<string, unknown>) {
  await run($, cwd, ['write', '--input', '-', ...callerArgs(caller)], JSON.stringify([block]))
}

/** Mirrors a task `TaskCreate` made; nothing where the agent has no active stage. */
export async function mirrorCreated($: EngineInterface, e: { subject?: unknown; agentId?: string }, ran: ToolCallResult) {
  const id = (ran.result as { task?: { id?: unknown } } | undefined)?.task?.id
  if (failed(ran) || typeof id !== 'string') return
  const cwd = await $.session.cwd()
  const caller = await callerOf($, e.agentId)
  const read = await peek($, cwd, caller)
  if (!succeeded(read)) return
  const stage = activeStage(read.doc, askerOf(caller))
  if (!stage) return
  const title = typeof e.subject === 'string' && e.subject !== '' ? e.subject : `Task ${id}`
  await write($, cwd, caller, { id: taskId(id, whoOf(caller)), kind: 'task', stage: stage.id, title, state: 'open' })
}

/** Mirrors a change `TaskUpdate` made to a task already mirrored. */
export async function mirrorUpdated(
  $: EngineInterface,
  e: { taskId?: unknown; subject?: unknown; status?: unknown; agentId?: string },
  ran: ToolCallResult,
) {
  if (failed(ran) || typeof e.taskId !== 'string') return
  if ((ran.result as { success?: unknown } | undefined)?.success === false) return
  const cwd = await $.session.cwd()
  const caller = await callerOf($, e.agentId)
  const read = await peek($, cwd, caller)
  if (!succeeded(read)) return
  const id = taskId(e.taskId, whoOf(caller))
  const held = blockById(read.doc, id)
  if (!held) return
  const body = held.block.version.body
  const title = typeof e.subject === 'string' && e.subject !== '' ? e.subject : String(body.title ?? id)
  const state = taskState(typeof e.status === 'string' ? e.status : undefined) ?? String(body.state ?? 'open')
  await write($, cwd, caller, { id, kind: 'task', stage: held.stage.id, title, state })
}
