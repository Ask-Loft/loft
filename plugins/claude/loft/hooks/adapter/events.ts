// The session's events, posted to the page server as they happen: turns, model requests, tool
// calls and subagents, each naming only what the page counts (the Feature "Record a session's
// events"). Plan mode and compactions are heard by the plugin's command hooks, since Claude Code's
// built-in guard keeps their events from a user's mod. The agent never waits on an event: neither
// on its post nor on reading what it names.

import type { EngineInterface, ModelUsage, On } from 'claude-code'

import type { ToolCallAnswer, ToolCallEvent, ToolCallNext, TurnCompleteEvent } from '../shared'
import { classOf, nearest, resolve, shellReads, type Class } from './consult'
import { HOST } from './contract'
import { flush, keep, timeOf, type Event } from './outbox'
import { known } from './server'
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

/**
 * The input field that names the file a reading tool reads, or the directory it searches, and
 * what it reads where the field is absent: a search runs in the agent's directory.
 */
const READ: Record<string, { field: string; otherwise?: string }> = {
  Read: { field: 'file_path' },
  NotebookRead: { field: 'notebook_path' },
  Grep: { field: 'path', otherwise: '.' },
  Glob: { field: 'path', otherwise: '.' },
}

/** What of a tool call's input decides what it consults, copied out of the input alone. */
type Consulting = { edited?: string; read?: string; pattern?: string; command?: string }

/** What decides what the tool call `e` consults, where it reads or edits files at all. */
function consulting(e: Record<string, unknown>): Consulting | undefined {
  const text = (field: string) => (typeof e[field] === 'string' ? (e[field] as string) : undefined)
  const tool = String(e.tool)
  if (EDITED[tool]) {
    const edited = text(EDITED[tool])
    return edited === undefined ? undefined : { edited }
  }
  if (tool === 'Bash') {
    const command = text('command')
    return command === undefined ? undefined : { command }
  }
  const reader = READ[tool]
  if (!reader) return undefined
  const read = text(reader.field) ?? reader.otherwise
  return read === undefined ? undefined : { read, pattern: tool === 'Glob' ? text('pattern') : undefined }
}

/** The directories a `Glob` pattern opens with, before its first wildcard. */
function literalHead(pattern: string): string {
  const parts = pattern.split('/')
  const wild = parts.findIndex(part => /[*?[{]/.test(part))
  return parts.slice(0, wild === -1 ? parts.length - 1 : wild).join('/')
}

/** What a tool call consults, as `reads` and `edits`, from the directory `dir` it runs in. */
function consultedBy({ edited, read, pattern, command }: Consulting, dir: string, trees: Iterable<string>): { reads?: Class[]; edits?: Class[] } {
  if (edited !== undefined) return { edits: [classOf(edited, dir, trees)] }
  if (command !== undefined) {
    const reads = shellReads(command, dir, trees)
    return reads.length > 0 ? { reads } : {}
  }
  let path = read!
  if (pattern !== undefined) {
    const head = literalHead(pattern)
    if (pattern.startsWith('/')) path = head || '/'
    else if (head !== '') path = `${path}/${head}`
  }
  return { reads: [classOf(path, dir, trees)] }
}

/**
 * Whether `path` lies under the `spec/` of the tree that holds it, where a known tree does: a
 * relative path only where the directory `dir` it is named from is known.
 */
export function isSpecEdit(path: string, trees: Iterable<string>, dir?: string): boolean | undefined {
  const absolute = path.startsWith('/') || dir !== undefined ? resolve(path, dir ?? '/') : undefined
  const within = absolute === undefined ? undefined : nearest(absolute, trees)
  return within === undefined || within === '' ? undefined : within.startsWith('spec/')
}

/** The events told so far: each is kept once those told before it are. */
let told: Promise<void> = Promise.resolve()

/**
 * Records one event of the calling agent, as it stands when told, and posts what is kept; the
 * caller waits on neither. An event whose fields cannot be read is dropped.
 */
export function tell($: EngineInterface, agentId: string | undefined, fields: Event | ((dir: string) => Event)) {
  if (!state.isOn) return
  const read = Promise.all([$.session.id(), $.session.cwd(), $.clock.now(), agentOf($, agentId)])
  told = Promise.all([told, read])
    .then(([, [session, dir, now, agent]]) => {
      const event = typeof fields === 'function' ? fields(dir) : fields
      // The descriptor goes with the session's first event and each turn's start only.
      const described = state.adapter !== undefined && (event.kind === 'turn.start' || !state.described.has(session))
      if (described) state.described.add(session)
      keep(state.box, { session, dir, at: timeOf(now), host: HOST, ...(described ? { adapter: state.adapter } : {}), ...(agent ? { agent } : {}), ...event })
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
  const input = consulting(e as Record<string, unknown>)
  const trees = [...known(state.server)]
  const spec = input?.edited === undefined ? undefined : isSpecEdit(input.edited, trees)
  const loft = input?.command === undefined ? [] : loftSubcommands(input.command)
  const start: Event = { kind: 'tool.start', tool: e.tool, call, ...(spec !== undefined ? { spec } : {}), ...(loft.length > 0 ? { loft } : {}) }
  // An edit's `spec` and `edits` then come from one path, resolved from the session's directory.
  const specFrom = (dir: string, trees: string[]) => {
    const resolved = input?.edited === undefined ? undefined : isSpecEdit(input.edited, trees, dir)
    const { spec: _, ...rest } = start
    return resolved === undefined ? rest : { ...rest, spec: resolved }
  }
  // Where the descriptor declares it, the start also says what the call consults, worked out
  // once the session's directory is read, so the call never waits on it.
  if (input !== undefined && state.adapter?.capabilities.consults === true) {
    tell($, e.agentId, dir => ({ ...specFrom(dir, trees), ...consultedBy(input, dir, trees) }) as Event)
  } else tell($, e.agentId, start)
  let failed = true
  try {
    const ran = await next(e)
    failed = ran.deny !== undefined || ran.isError === true
    return ran
  } finally {
    tell($, e.agentId, { kind: 'tool.end', tool: e.tool, call, failed })
  }
}
