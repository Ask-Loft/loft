// What Yard Client's hooks keep while the module is loaded: the settings, whether Yard Client is
// on, the session's chain of ids, the page server, the events not yet posted. A reload starts it
// over, as it starts the pane's over. And the calls every hook makes through it: who is calling,
// and running `loft yard` in their directory.

import { atom, read, update } from 'claude-code'
import type { EngineInterface, PluginOptions } from 'claude-code'

import { settingsOf, type Ran, type Settings } from '../loft'
import { outboxOf, type Outbox } from './outbox'
import { pageOf, type Asker } from './log'
import { callerArgs, succeeded, yard, type Caller } from './run'
import { register as registerTree, serverOf, type Server } from './server'

/** The plugin's name, as Claude Code names it in a dispatch's origin and in its tools. */
export const PLUGIN = 'loft'

/** The page's address for the branch's Yard Log, which a dialog shows above what it asks. */
const page = atom({ plugin: 'loft', key: 'yardPage' } as const, null)

/** The page's address, as the last `loft yard` run gave it; null until one does. */
export async function pageNoted($: EngineInterface) {
  return read($, page)
}

/** How the person decided a plan. */
export type Outcome = { verdict?: unknown; reason?: unknown; channel?: unknown }

/**
 * The plan Claude Code is asking the person to approve, while it is asked: where it was sent, the
 * end of the wait the permission request runs for it, and the page's outcome once the page decided.
 */
export type Presenting = { id: string; cwd: string; caller: Caller; submission: string; end?: () => void; decided?: Outcome }

export type YardState = {
  settings: Settings
  server: Server
  box: Outbox
  /** Each subagent's type, by its id. */
  types: Map<string, string>
  /** This process's sessions, the current one first: a `/clear` or a resume starts another. */
  sessions: string[]
  /** The session that ended with `/clear` or a resume, until `loft yard continue` records it. */
  continues: string | undefined
  isOn: boolean
  enabling: Promise<boolean> | undefined
  presenting: Presenting | null
}

function fresh(options: PluginOptions): YardState {
  const settings = settingsOf(options)
  return {
    settings,
    server: serverOf(),
    box: outboxOf(settings.port),
    types: new Map(),
    sessions: [],
    continues: undefined,
    isOn: false,
    enabling: undefined,
    presenting: null,
  }
}

export const state: YardState = fresh({})

/** Starts the state over for a module just loaded with these options. */
export function reset(options: PluginOptions) {
  Object.assign(state, fresh(options))
}

/** The calling agent, once the session it may continue is recorded. */
export async function callerOf($: EngineInterface, agentId: string | undefined): Promise<Caller> {
  const session = await continued($)
  const agent = await agentOf($, agentId)
  return { session, ...(agent ? { agent } : {}) }
}

/**
 * The session now, after recording once, with `loft yard continue`, that it continues the one
 * that ended with `/clear` or a resume: before any other `loft yard` command it runs.
 */
export async function continued($: EngineInterface): Promise<string> {
  const session = await $.session.id()
  if (state.sessions[0] !== session) state.sessions.unshift(session)
  const ended = state.continues
  if (ended !== undefined && ended !== session) {
    state.continues = undefined
    await yard($, state.settings, await $.session.cwd(), ['continue', '--session', session, '--from', ended], { beyond: false })
  }
  return session
}

/** Who asks, for matching the records Yard Log keeps: the session's ids and the subagent's. */
export function askerOf(caller: Caller): Asker {
  return {
    sessions: [caller.session, ...state.sessions.filter(each => each !== caller.session)],
    ...(caller.agent ? { agent: caller.agent.id } : {}),
  }
}

/** Runs `loft yard <args>` in `cwd`, noting the page's address and any tree not yet served. */
export async function run($: EngineInterface, cwd: string, args: readonly string[], stdin?: string): Promise<Ran> {
  const ran = await yard($, state.settings, cwd, args, stdin === undefined ? {} : { stdin })
  const url = pageOf(ran.doc)
  if (url) await update($, page, () => url)
  const tree = ran.doc?.tree
  if (state.isOn && typeof tree === 'string' && !state.server.trees.has(tree)) {
    void registerTree($, state.settings, state.server, state.sessions[0] ?? '', tree)
  }
  return ran
}

/**
 * Reads Yard Log on the agent's behalf: with no `--mark-read`, so the revision it last read stays
 * as it was, as the mod reads it to mirror a task or to send what the agent asks.
 */
export async function peek($: EngineInterface, cwd: string, caller: Caller): Promise<Ran> {
  return run($, cwd, ['read', ...callerArgs(caller)])
}

/**
 * Where the calling agent works, who it is, and Yard Log as the mod peeks at it on their behalf;
 * undefined where Yard Log cannot be read.
 */
export async function prepare($: EngineInterface, agentId: string | undefined) {
  const cwd = await $.session.cwd()
  const caller = await callerOf($, agentId)
  const read = await peek($, cwd, caller)
  return succeeded(read) ? { cwd, caller, doc: read.doc } : undefined
}

/** A subagent's type, as Claude Code lists it. */
export async function typeOf($: EngineInterface, agentId: string): Promise<string> {
  try {
    return (await $.agent.list()).find(agent => agent.id === agentId)?.type ?? 'subagent'
  } catch {
    return 'subagent'
  }
}

/** The agent an event names: a subagent by its id and type; the main agent names none. */
export async function agentOf($: EngineInterface, agentId: string | undefined) {
  return agentId ? { id: agentId, type: state.types.get(agentId) ?? (await typeOf($, agentId)) } : undefined
}
