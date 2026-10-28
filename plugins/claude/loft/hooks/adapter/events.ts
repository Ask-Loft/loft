// The session's events, posted to the page server as they happen: turns, model requests, tool
// calls and subagents, each naming only what the page counts (the Feature "Record a session's
// events"). Plan mode and compactions are heard by the plugin's command hooks, since Claude Code's
// built-in guard keeps their events from a user's mod. The agent never waits on an event: neither
// on its post nor on reading what it names.

import type { EngineInterface, ModelUsage, On } from 'claude-code'

import { relativeTo } from '../model'
import type { ToolCallAnswer, ToolCallEvent, ToolCallNext, TurnCompleteEvent } from '../shared'
import { HOST } from './contract'
import { flush, keep, timeOf, type Event } from './outbox'
import { loftSubcommands } from './shell'
import { actorKey, agentOf, continued, PLUGIN, state } from './state'
import { endAssigned } from './task-writes'

/** Model usage as the page counts it. */
export function tokensOf(usage: ModelUsage) {
  return {
    input: usage.input_tokens,
    cacheRead: usage.cache_read_input_tokens,
    cacheWrite: usage.cache_creation_input_tokens,
    output: usage.output_tokens,
  }
}

/** The input field that names a file edit's file, by tool. */
const EDITED: Record<string, string> = { Write: 'file_path', Edit: 'file_path', MultiEdit: 'file_path', NotebookEdit: 'notebook_path' }

/** Whether `path` lies under the `spec/` of the tree that holds it, where a known tree does. */
export function isSpecEdit(path: string, trees: Iterable<string>): boolean | undefined {
  // The nearest tree is the one that leaves the shortest path.
  const relative = [...trees].flatMap(top => relativeTo(top, path) ?? []).sort((a, b) => a.length - b.length)[0]
  return relative === undefined ? undefined : relative.startsWith('spec/')
}

/** The events told so far: each is kept once those told before it are. */
let told: Promise<void> = Promise.resolve()

/**
 * Records one event of the calling agent, as it stands when told, and posts what is kept; the
 * caller waits on neither. An event whose fields cannot be read is dropped.
 */
export function tell($: EngineInterface, agentId: string | undefined, fields: Event) {
  if (!state.isOn) return
  const read = Promise.all([$.session.id(), $.session.cwd(), $.clock.now(), agentOf($, agentId)])
  told = Promise.all([told, read])
    .then(([, [session, dir, now, agent]]) => {
      // The descriptor goes with the session's first event and each turn's start only.
      const described = state.adapter !== undefined && (fields.kind === 'turn.start' || !state.described.has(session))
      if (described) state.described.add(session)
      keep(state.box, { session, dir, at: timeOf(now), host: HOST, ...(described ? { adapter: state.adapter } : {}), ...(agent ? { agent } : {}), ...fields })
      void flush($, state.box)
    })
    .catch(() => {})
}

/** Registers the hooks that record the session's events, but those the entry hooks for both parts. */
export function registerEvents(on: On) {
  told = Promise.resolve()

  on('turn.start', async ($, e, next) => {
    // The main loop's turn names the session now, so that after `/clear`, which no session start
    // follows, the session's captures run in its own lane.
    if (!e.agentId && state.isOn) await continued($).catch(() => {})
    state.endedTasks.delete(actorKey(state.sessions[0] ?? await $.session.id(), e.agentId))
    tell($, e.agentId, { kind: 'turn.start', turn: e.turnId })
    return next(e)
  })

  on('turn.step', async function* ($, e, next) {
    const response = yield* next(e)
    if (response?.usage) {
      tell($, e.agentId, {
        kind: 'request',
        turn: e.turnId,
        model: response.usage.model,
        tools: response.toolUses.map(use => use.name),
        tokens: tokensOf(response.usage),
      })
    }
    return response
  })

  // A subagent's start: the type it was spawned as and the model it runs on.
  on('agent.spawn', async ($, e, next) => {
    const spawned = await next(e)
    const id = spawned.deny === undefined ? spawned.agentId : undefined
    if (id === undefined) return spawned
    state.endedTasks.delete(actorKey(state.sessions[0] ?? await $.session.id(), id))
    state.types.set(id, e.subagentType)
    tell($, undefined, { kind: 'subagent.start', agent: { id, type: e.subagentType }, model: spawned.model })
    return spawned
  })
}

/** A turn's end, posted once Claude Code has ended it. */
export async function eventsOfTurnEnd($: EngineInterface, e: TurnCompleteEvent) {
  tell($, e.agentId, { kind: 'turn.end', turn: e.turnId, ...(e.usage ? { tokens: tokensOf(e.usage) } : {}) })
  // Each run of a subagent's loop is one turn, so its turn's end is the subagent's. The main
  // agent goes on after its turn: only an interruption, or its session's end, ends its work.
  if (e.agentId) tell($, e.agentId, { kind: 'subagent.end' })
  if (e.agentId || e.isAborted) await endAssigned($, e.agentId).catch(() => {})
}

/** A tool call's start and end; the mod's own dialogs are not the agent's calls. */
export async function eventsOfToolCall($: EngineInterface, e: ToolCallEvent, next: ToolCallNext): Promise<ToolCallAnswer> {
  if (next.origin.plugin === PLUGIN) return next(e)
  const call = e.tool_use_id ?? ''
  const field = EDITED[String(e.tool)]
  const path = field ? (e as Record<string, unknown>)[field] : undefined
  const spec = typeof path === 'string' ? isSpecEdit(path, state.server.trees) : undefined
  const command = e.tool === 'Bash' ? String((e as { command?: unknown }).command ?? '') : ''
  const loft = command === '' ? [] : loftSubcommands(command)
  tell($, e.agentId, { kind: 'tool.start', tool: e.tool, call, ...(spec !== undefined ? { spec } : {}), ...(loft.length > 0 ? { loft } : {}) })
  let failed = true
  try {
    const ran = await next(e)
    failed = ran.deny !== undefined || ran.isError === true
    return ran
  } finally {
    tell($, e.agentId, { kind: 'tool.end', tool: e.tool, call, failed })
  }
}
