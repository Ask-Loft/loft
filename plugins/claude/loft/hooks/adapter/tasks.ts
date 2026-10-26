// The agent's own tasks, mirrored as `task` blocks of the stage active for the agent that made them
// (the Feature "Mirror the agent's tasks"). The agent's call never waits on a mirror; the mirrors
// run one after another, so that an update finds the block its task's creation wrote.

import type { EngineInterface, ToolCallResult } from 'claude-code'

import { taskId, taskState, taskType } from './answers'
import { activeStage, blockById, whoOf } from './log'
import { callerArgs, type Caller } from './run'
import { askerOf, prepare, run } from './state'

const failed = (ran: ToolCallResult) => ran.deny !== undefined || ran.isError === true

/** The mirrors under way, the last one queued at its end. */
let mirroring: Promise<void> = Promise.resolve()

/** Runs a mirror once those queued before it are done; a mirror that fails is dropped. */
function queue(mirror: () => Promise<void>) {
  mirroring = mirroring.then(mirror).catch(() => {})
}

async function write($: EngineInterface, cwd: string, caller: Caller, block: Record<string, unknown>) {
  await run($, cwd, ['write', '--input', '-', ...callerArgs(caller)], JSON.stringify([block]))
}

/** A task's type as its block holds it: a regular task's block names none. */
const typed = (type: string | undefined) => (type === undefined || type === 'regular' ? {} : { type })

/** Mirrors a task `TaskCreate` made; nothing where the agent has no active stage. */
export function mirrorCreated(
  $: EngineInterface,
  e: { subject?: unknown; metadata?: unknown; agentId?: string },
  ran: ToolCallResult,
) {
  queue(() => created($, e, ran))
}

/** Mirrors a change `TaskUpdate` made to a task already mirrored. */
export function mirrorUpdated(
  $: EngineInterface,
  e: { taskId?: unknown; subject?: unknown; status?: unknown; metadata?: unknown; agentId?: string },
  ran: ToolCallResult,
) {
  queue(() => updated($, e, ran))
}

async function created(
  $: EngineInterface,
  e: { subject?: unknown; metadata?: unknown; agentId?: string },
  ran: ToolCallResult,
) {
  const id = (ran.result as { task?: { id?: unknown } } | undefined)?.task?.id
  if (failed(ran) || typeof id !== 'string') return
  const asking = await prepare($, e.agentId)
  if (!asking) return
  const { cwd, caller, doc } = asking
  const stage = activeStage(doc, askerOf(caller))
  if (!stage) return
  const title = typeof e.subject === 'string' && e.subject !== '' ? e.subject : `Task ${id}`
  const block = { id: taskId(id, whoOf(caller)), kind: 'task', stage: stage.id, title, state: 'open' }
  await write($, cwd, caller, { ...block, ...typed(taskType(e.metadata)) })
}

async function updated(
  $: EngineInterface,
  e: { taskId?: unknown; subject?: unknown; status?: unknown; metadata?: unknown; agentId?: string },
  ran: ToolCallResult,
) {
  if (failed(ran) || typeof e.taskId !== 'string') return
  if ((ran.result as { success?: unknown } | undefined)?.success === false) return
  const asking = await prepare($, e.agentId)
  if (!asking) return
  const { cwd, caller, doc } = asking
  const id = taskId(e.taskId, whoOf(caller))
  const held = blockById(doc, id)
  if (!held) return
  const body = held.block.version.body
  const title = typeof e.subject === 'string' && e.subject !== '' ? e.subject : String(body.title ?? id)
  const state = taskState(typeof e.status === 'string' ? e.status : undefined) ?? String(body.state ?? 'open')
  // The block is written whole, so what the update leaves alone is written as it was.
  const detail = typeof body.detail === 'string' ? { detail: body.detail } : {}
  const type = taskType(e.metadata) ?? (typeof body.type === 'string' ? body.type : undefined)
  await write($, cwd, caller, { id, kind: 'task', stage: held.stage.id, title, ...detail, state, ...typed(type) })
}
