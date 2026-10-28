// Optimistic task writes preserve the current body and assignment on every retry.
import type { EngineInterface } from 'claude-code'

import type { Doc } from '../loft'
import { isObject } from '../model'
import { isAsker, stagesOf, type Actor, type Asker } from './log'
import { callerArgs, succeeded, type Caller } from './run'
import { peek, prepare, run, state } from './state'

export type Prepared = Awaited<ReturnType<typeof prepare>>
export const actorKey = (caller: Caller) => JSON.stringify([caller.session, caller.agent?.id ?? null])
export const exactCaller = (caller: Caller): Asker => ({ sessions: [caller.session], agent: caller.agent?.id })

export function assignedTo(body: Record<string, unknown>, asker: Asker): boolean {
  return isObject(body.assignee) && isAsker(body.assignee as Actor, asker)
}

export function assignedTasks(doc: Doc, asker: Asker) {
  return stagesOf(doc).flatMap(stage => (stage.blocks ?? [])
    .filter(block => block.kind === 'task' && assignedTo(block.version.body, asker))
    .map(block => ({ stage, block })))
}

export function normalized(body: Record<string, unknown>): Record<string, unknown> {
  const copy = { ...body }
  if (copy.type === 'regular') delete copy.type
  return copy
}

export async function writeTasks(
  $: EngineInterface, asking: Prepared, select: (doc: Doc) => Record<string, unknown>[],
  cancelled: () => boolean = () => false,
) {
  if (!asking) return
  const { cwd, caller } = asking
  let doc = asking.doc
  for (let attempt = 0; attempt < 3; attempt++) {
    if (cancelled()) return
    const batch = select(doc)
    if (batch.length === 0) return
    const written = await run($, cwd, ['write', '--input', '-', '--based-on', String(doc.revision), ...callerArgs(caller)], JSON.stringify(batch))
    if (succeeded(written)) return
    const errors = written.doc?.errors
    if (!Array.isArray(errors) || !errors.some(error => isObject(error) && error.kind === 'stale')) return
    const latest = await peek($, cwd, caller)
    if (!succeeded(latest)) return
    doc = latest.doc
  }
}

/** Only the reliably identified actor's active assignments are returned to open. */
export async function endAssigned($: EngineInterface, agentId: string | undefined) {
  if (!state.isOn) return
  if (state.sessions[0]) state.endedTasks.add(actorKey({ session: state.sessions[0], ...(agentId ? { agent: { id: agentId, type: '' } } : {}) }))
  const asking = await prepare($, agentId)
  if (!asking) return
  state.endedTasks.add(actorKey(asking.caller))
  await writeTasks($, asking, doc => assignedTasks(doc, exactCaller(asking.caller))
    .filter(({ block }) => block.version.body.state === 'active')
    .map(({ stage, block }) => ({ ...normalized(block.version.body), id: block.id, kind: 'task', stage: stage.id, state: 'open' })))
}
