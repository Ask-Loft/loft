// The session's events, posted to the page server as they happen: turns, model requests, tool
// calls, subagents, compactions and plan mode, each naming only what the page counts (the Feature
// "Record a session's events"). The agent never waits on an event: neither on its post nor on
// reading what it names.

import type { EngineInterface, ModelUsage, On } from 'claude-code'

import { relativeTo } from '../model'
import type { ToolCallAnswer, ToolCallEvent, ToolCallNext, TurnCompleteEvent } from '../shared'
import { HOST } from './contract'
import { flush, keep, timeOf, type Event } from './outbox'
import { loftSubcommands } from './shell'
import { actorKey, agentOf, PLUGIN, state } from './state'
import { contextFor } from './context'
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

/** Whether the main agent is in plan mode, as last heard; null until first heard. */
const mode: { isPlanning: boolean | null } = { isPlanning: null }

/** Plan mode starting or ending, once for each change; a session first heard out of it says nothing. */
function planMode($: EngineInterface, isPlanning: boolean) {
  const was = mode.isPlanning
  mode.isPlanning = isPlanning
  if (was === isPlanning || (was === null && !isPlanning)) return
  tell($, undefined, { kind: isPlanning ? 'plan.start' : 'plan.end' })
}

/** The main agent's permission mode, as each classic hook is given it. */
function heard($: EngineInterface, e: { agent_id?: string; permission_mode?: string }) {
  if (!e.agent_id && e.permission_mode) planMode($, e.permission_mode === 'plan')
}

/** Subagents that started and whose start waits for the model their first request names. */
const starting = new Map<string, { at: string; type: string }>()

/** Registers the hooks that record the session's events, but those the entry hooks for both parts. */
export function registerEvents(on: On) {
  mode.isPlanning = null
  starting.clear()
  told = Promise.resolve()

  on('turn.start', async ($, e, next) => {
    state.endedTasks.delete(actorKey(state.sessions[0] ?? await $.session.id(), e.agentId))
    tell($, e.agentId, { kind: 'turn.start', turn: e.turnId })
    return next(e)
  })

  on('turn.step', async function* ($, e, next) {
    const response = yield* next(e)
    const start = e.agentId ? starting.get(e.agentId) : undefined
    if (e.agentId && start) {
      starting.delete(e.agentId)
      tell($, undefined, { kind: 'subagent.start', at: start.at, agent: { id: e.agentId, type: start.type }, model: e.model })
    }
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

  on('classic.SubagentStart', async ($, e, next) => {
    state.endedTasks.delete(actorKey(state.sessions[0] ?? e.session_id, e.agent_id))
    state.types.set(e.agent_id, e.agent_type)
    starting.set(e.agent_id, { at: timeOf(await $.clock.now()), type: e.agent_type })
    const started = await next(e)
    const text = state.isOn ? await contextFor($, await $.session.cwd(), e.agent_id).catch(() => undefined) : undefined
    if (text === undefined) return started
    return { ...started, additionalContext: [...(started.additionalContext ?? []), text] }
  })

  on('classic.SubagentStop', async ($, e, next) => {
    starting.delete(e.agent_id)
    tell($, e.agent_id, { kind: 'subagent.end' })
    return next(e)
  })

  on('session.compact', async ($, e, next) => {
    const compacted = await next(e)
    if (compacted.skip === undefined) {
      tell($, undefined, {
        kind: 'compaction',
        ...(compacted.tokensBefore !== undefined ? { before: compacted.tokensBefore } : {}),
        ...(compacted.tokensAfter !== undefined ? { after: compacted.tokensAfter } : {}),
        ...(compacted.usage ? { tokens: tokensOf(compacted.usage) } : {}),
      })
    }
    return compacted
  })

  // Claude Code gives each classic hook the permission mode, however plan mode began or ended.
  on('classic.UserPromptSubmit', async ($, e, next) => {
    state.endedTasks.delete(actorKey(state.sessions[0] ?? e.session_id))
    heard($, e)
    return next(e)
  })
  on('classic.PostToolUse', async ($, e, next) => {
    heard($, e)
    return next(e)
  })
  on('classic.Stop', async ($, e, next) => {
    heard($, e)
    return next(e)
  })
}

/** A turn's end, posted once Claude Code has ended it. */
export async function eventsOfTurnEnd($: EngineInterface, e: TurnCompleteEvent) {
  tell($, e.agentId, { kind: 'turn.end', turn: e.turnId, ...(e.usage ? { tokens: tokensOf(e.usage) } : {}) })
  await endAssigned($, e.agentId).catch(() => {})
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
    if (!failed && !e.agentId && (e.tool === 'EnterPlanMode' || e.tool === 'ExitPlanMode')) {
      planMode($, e.tool === 'EnterPlanMode')
    }
  }
}
