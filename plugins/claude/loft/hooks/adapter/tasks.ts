// Native task identities stay local; explicit Yard references continue an existing task.
import type { EngineInterface, ToolCallResult } from 'claude-code'

import { isObject } from '../model'
import { taskId, taskState, taskType } from './answers'
import { activeStage, blockById, isAsker, whoOf } from './log'
import { askerOf, state } from './state'
import { actorKey, assignedTo, exactCaller, normalized, writeTasks, type Prepared } from './task-writes'

const failed = (ran: ToolCallResult) => ran.deny !== undefined || ran.isError === true
const binding = (metadata: unknown): string | null | undefined => {
  if (!isObject(metadata) || !('yardTask' in metadata)) return undefined
  return typeof metadata.yardTask === 'string' && /^[a-z][a-z0-9-]*$/.test(metadata.yardTask) ? metadata.yardTask : null
}

type Change = { taskId?: unknown; subject?: unknown; description?: unknown; status?: unknown; metadata?: unknown; agentId?: string }

export async function mirrorCreated($: EngineInterface, e: Change, ran: ToolCallResult, asking: Prepared, signal?: AbortSignal) {
  const id = (ran.result as { task?: { id?: unknown } } | undefined)?.task?.id
  if (failed(ran) || typeof id !== 'string' || !asking || binding(e.metadata) !== undefined) return
  const { caller, doc } = asking
  const stage = activeStage(doc, askerOf(caller))
  if (!stage) return
  const title = typeof e.subject === 'string' && e.subject !== '' ? e.subject : `Task ${id}`
  const type = taskType(e.metadata)
  const block = { id: taskId(id, whoOf(caller)), kind: 'task', stage: stage.id, title, state: 'open',
    ...(type && type !== 'regular' ? { type } : {}) }
  await writeTasks($, asking, () => [block], () => signal?.aborted === true)
}

export async function mirrorUpdated($: EngineInterface, e: Change, ran: ToolCallResult, asking: Prepared, signal?: AbortSignal) {
  if (failed(ran) || typeof e.taskId !== 'string' || !asking) return
  if ((ran.result as { success?: unknown } | undefined)?.success === false) return
  const bound = binding(e.metadata)
  if (bound === null) return
  const caller = asking.caller
  const id = bound ?? taskId(e.taskId, whoOf(caller))
  await writeTasks($, asking, doc => {
    const held = blockById(doc, id)
    if (!held || held.block.kind !== 'task') return []
    const body = held.block.version.body
    if ((bound || isObject(body.assignee)) && state.endedTasks.has(actorKey(caller))) return []
    const actor = exactCaller(caller)
    if (isObject(body.assignee) && !assignedTo(body, actor)) return []
    if (bound && !isObject(body.assignee) && !isAsker(held.stage.owner, actor)) return []
    const next = normalized(body)
    if (typeof e.subject === 'string' && e.subject !== '') next.title = e.subject
    if (typeof e.description === 'string') next.detail = e.description
    const status = taskState(typeof e.status === 'string' ? e.status : undefined)
    if (status && !(bound && ['done', 'dropped'].includes(String(body.state)))) next.state = status
    const type = taskType(e.metadata)
    if (type === 'regular') delete next.type
    else if (type) next.type = type
    if (bound && JSON.stringify(next) === JSON.stringify(normalized(body))) return []
    return [{ ...next, id, kind: 'task', stage: held.stage.id }]
  }, () => signal?.aborted === true)
}
