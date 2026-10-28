// Native task identities stay local; explicit Yard references continue an existing task. A new
// unbound task goes to the agent's working stage, and every native update is mirrored with
// `loft yard task --mirror`, which applies the delegation rules once, in Yard Log.
import type { EngineInterface, ToolCallResult } from 'claude-code'

import type { Doc } from '../loft'
import { isObject } from '../model'
import { taskId, taskState, taskType } from './answers'
import { HOST } from './contract'
import { isAsker, stagesOf, whoOf } from './log'
import { callerArgs, type Caller } from './run'
import { actorKey, callerOf, run, state, type prepare } from './state'

type Prepared = Awaited<ReturnType<typeof prepare>>

const failed = (ran: ToolCallResult) => ran.deny !== undefined || ran.isError === true
const binding = (metadata: unknown): string | null | undefined => {
  if (!isObject(metadata) || !('yardTask' in metadata)) return undefined
  return typeof metadata.yardTask === 'string' && /^[a-z][a-z0-9-]*$/.test(metadata.yardTask) ? metadata.yardTask : null
}

type Change = { taskId?: unknown; subject?: unknown; description?: unknown; status?: unknown; metadata?: unknown; agentId?: string }

/** The agent's working stage, as its briefing names it, and whether the agent owns it. */
function workingStage(doc: Doc, caller: Caller): { id: string; owned: boolean } | undefined {
  const context = isObject(doc.context) ? doc.context : undefined
  const stage = isObject(context?.stage) ? context.stage : undefined
  if (typeof stage?.id !== 'string') return undefined
  const owner = stagesOf(doc).find(each => each.id === stage.id)?.owner
  return { id: stage.id, owned: isAsker(owner, { sessions: [caller.session], agent: caller.agent?.id }) }
}

/** The caller as a task's assignee. */
const assigneeOf = (caller: Caller) => ({
  host: HOST,
  session: caller.session,
  ...(caller.agent ? { agent: caller.agent.id, agentType: caller.agent.type } : {}),
})

/**
 * Writes an unbound native task as a task block of the working stage its briefing named before
 * the native creation ran, assigned to the agent where the agent does not own that stage.
 */
export async function mirrorCreated($: EngineInterface, e: Change, ran: ToolCallResult, asking: Prepared, signal?: AbortSignal) {
  const id = (ran.result as { task?: { id?: unknown } } | undefined)?.task?.id
  if (failed(ran) || typeof id !== 'string' || !asking || binding(e.metadata) !== undefined) return
  const { cwd, caller, doc } = asking
  const stage = workingStage(doc, caller)
  if (!stage || signal?.aborted) return
  const title = typeof e.subject === 'string' && e.subject !== '' ? e.subject : `Task ${id}`
  const type = taskType(e.metadata)
  const block = { id: taskId(id, whoOf(caller)), kind: 'task', stage: stage.id, title, state: 'open',
    ...(type && type !== 'regular' ? { type } : {}), ...(stage.owned ? {} : { assignee: assigneeOf(caller) }) }
  await run($, cwd, ['write', '--input', '-', ...callerArgs(caller)], JSON.stringify([block]))
}

/** The `loft yard task` flags of the fields a native update changes. */
function flagsOf(e: Change): string[] {
  const state = taskState(typeof e.status === 'string' ? e.status : undefined)
  const type = taskType(e.metadata)
  return [
    ...(state ? ['--state', state] : []),
    ...(typeof e.subject === 'string' && e.subject !== '' ? [`--title=${e.subject}`] : []),
    ...(typeof e.description === 'string' ? [`--detail=${e.description}`] : []),
    ...(type ? ['--type', type] : []),
  ]
}

/** Mirrors a native update onto its task, bound or minted, without reading it first. */
export async function mirrorUpdated($: EngineInterface, e: Change, ran: ToolCallResult, signal?: AbortSignal) {
  if (failed(ran) || typeof e.taskId !== 'string') return
  if ((ran.result as { success?: unknown } | undefined)?.success === false) return
  const bound = binding(e.metadata)
  const flags = flagsOf(e)
  if (bound === null || flags.length === 0) return
  const caller = await callerOf($, e.agentId)
  if (state.endedTasks.has(actorKey(caller.session, e.agentId)) || signal?.aborted) return
  const id = bound ?? taskId(e.taskId, whoOf(caller))
  await run($, await $.session.cwd(), ['task', id, '--mirror', ...flags, ...callerArgs(caller)])
}
