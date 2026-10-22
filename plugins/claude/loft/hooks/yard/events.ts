// The session's events, posted to the page server as they happen: turns, model requests, tool
// calls, subagents, compactions and plan mode, each naming only what the page counts (the Feature
// "Record a session's events"). The agent never waits on a post.

import type { EngineInterface, ModelUsage, On } from 'claude-code'

import type { Onward, ToolCallAnswer, ToolCallEvent, ToolCallNext, TurnCompleteEvent } from '../shared'
import { flush, keep, timeOf, type Event } from './outbox'
import { loftSubcommands } from './shell'
import { agentOf, PLUGIN, state } from './state'

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
  const tree = [...trees].filter(top => path.startsWith(`${top}/`)).sort((a, b) => b.length - a.length)[0]
  return tree === undefined ? undefined : path.startsWith(`${tree}/spec/`)
}

/** Records one event of the calling agent and posts what is kept, without waiting on the post. */
export async function tell($: EngineInterface, agentId: string | undefined, fields: Event) {
  if (!state.isOn) return
  const [session, dir, now, agent] = await Promise.all([$.session.id(), $.session.cwd(), $.clock.now(), agentOf($, agentId)])
  keep(state.box, { session, dir, at: timeOf(now), host: 'claude-code', ...(agent ? { agent } : {}), ...fields })
  void flush($, state.box)
}

/** Whether the main agent is in plan mode, as last heard; null until first heard. */
const mode: { isPlanning: boolean | null } = { isPlanning: null }

/** Plan mode starting or ending, once for each change; a session first heard out of it says nothing. */
async function planMode($: EngineInterface, isPlanning: boolean) {
  const was = mode.isPlanning
  mode.isPlanning = isPlanning
  if (was === isPlanning || (was === null && !isPlanning)) return
  await tell($, undefined, { kind: isPlanning ? 'plan.start' : 'plan.end' })
}

/** The main agent's permission mode, as each classic hook is given it. */
async function heard($: EngineInterface, e: { agent_id?: string; permission_mode?: string }) {
  if (!e.agent_id && e.permission_mode) await planMode($, e.permission_mode === 'plan')
}

/** Subagents that started and whose start waits for the model their first request names. */
const starting = new Map<string, { at: string; type: string }>()

/** Registers the hooks that record the session's events, but those the entry hooks for both parts. */
export function registerEvents(on: On) {
  mode.isPlanning = null
  starting.clear()

  on('turn.start', async ($, e, next) => {
    await tell($, e.agentId, { kind: 'turn.start', turn: e.turnId })
    return next(e)
  })

  on('turn.step', async function* ($, e, next) {
    const response = yield* next(e)
    const start = e.agentId ? starting.get(e.agentId) : undefined
    if (e.agentId && start) {
      starting.delete(e.agentId)
      await tell($, undefined, { kind: 'subagent.start', at: start.at, agent: { id: e.agentId, type: start.type }, model: e.model })
    }
    if (response?.usage) {
      await tell($, e.agentId, {
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
    state.types.set(e.agent_id, e.agent_type)
    starting.set(e.agent_id, { at: timeOf(await $.clock.now()), type: e.agent_type })
    return next(e)
  })

  on('classic.SubagentStop', async ($, e, next) => {
    starting.delete(e.agent_id)
    await tell($, e.agent_id, { kind: 'subagent.end' })
    return next(e)
  })

  on('session.compact', async ($, e, next) => {
    const compacted = await next(e)
    if (compacted.skip === undefined) {
      await tell($, undefined, {
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
    await heard($, e)
    return next(e)
  })
  on('classic.PostToolUse', async ($, e, next) => {
    await heard($, e)
    return next(e)
  })
  on('classic.Stop', async ($, e, next) => {
    await heard($, e)
    return next(e)
  })
}

/** A turn's end, posted once Claude Code has ended it. */
export async function eventsOfTurnEnd($: EngineInterface, e: TurnCompleteEvent) {
  await tell($, e.agentId, { kind: 'turn.end', turn: e.turnId, ...(e.usage ? { tokens: tokensOf(e.usage) } : {}) })
}

/** A tool call's start and end; the mod's own dialogs are not the agent's calls. */
export async function eventsOfToolCall($: EngineInterface, e: ToolCallEvent, onward: Onward, next: ToolCallNext): Promise<ToolCallAnswer> {
  if (next.origin.plugin === PLUGIN) return onward(e)
  const call = e.tool_use_id ?? ''
  const field = EDITED[String(e.tool)]
  const path = field ? (e as Record<string, unknown>)[field] : undefined
  const spec = typeof path === 'string' ? isSpecEdit(path, state.server.trees) : undefined
  const command = e.tool === 'Bash' ? String((e as { command?: unknown }).command ?? '') : ''
  const loft = command === '' ? [] : loftSubcommands(command)
  await tell($, e.agentId, { kind: 'tool.start', tool: e.tool, call, ...(spec !== undefined ? { spec } : {}), ...(loft.length > 0 ? { loft } : {}) })
  let failed = true
  try {
    const ran = await onward(e)
    failed = ran.deny !== undefined || ran.isError === true
    return ran
  } finally {
    await tell($, e.agentId, { kind: 'tool.end', tool: e.tool, call, failed })
    if (!failed && !e.agentId && (e.tool === 'EnterPlanMode' || e.tool === 'ExitPlanMode')) {
      await planMode($, e.tool === 'EnterPlanMode')
    }
  }
}
