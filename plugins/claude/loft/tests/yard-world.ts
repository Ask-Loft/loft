// The world beneath Yard Client's hooks, answered from memory: `loft yard` as the plugin's
// `loftCommand` runs it, the page server's events and beat, the token file, the host's dialogs and
// tools. Each run, post and dialog is recorded for the tests to read.

import { mock, test } from 'claude-code/testing'

import { idOf, type Block, type Stage } from '../hooks/adapter/log'

type Body = Extract<Parameters<typeof test>[1], (...args: never[]) => unknown>
export type Engine = Parameters<Body>[0]
export type On = Parameters<Body>[1]

export const TREE = '/work/shelf'
export const HOME = '/home/reader'
export const SESSION = 'session-1'
export const PAGE = 'http://127.0.0.1:7411/#tree=%2Fwork%2Fshelf&branch=feat%2Fshelf'

type Doc = Record<string, unknown>

/** A block as `loft yard` documents give it. */
export function block(id: string, kind: string, body: Record<string, unknown>): Block {
  return { id, key: `k-${id}`, kind, version: { body } }
}

/** A stage with its owner, holding no blocks. */
export function stage(id: string, status: string, owner: Stage['owner']): Stage {
  return { ...block(id, 'stage', { title: `Stage ${id}`, goal: '', status }), owner, blocks: [] }
}

/** A `loft yard read` document of an open Yard Log, with these fields over it. */
export function readDoc(fields: Doc = {}): Doc {
  return {
    schema: 'loft-yard-read/1',
    outcome: 'read',
    errors: [],
    tree: TREE,
    branch: 'feat/shelf',
    revision: 4,
    lastRead: 4,
    ignored: false,
    page: { available: true, port: 7411, url: PAGE },
    yardLog: { key: 'yard-log-1', closedBy: 'open' },
    stages: [],
    submissions: [],
    ...fields,
  }
}

/** A promise resolved from outside. */
export function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(settle => (resolve = settle))
  return { promise, resolve }
}

const sameActor = (actor: unknown, session: string | undefined, agent: string | undefined) => {
  const named = (actor ?? {}) as { session?: string; agent?: string }
  return named.session === session && (named.agent ?? undefined) === agent
}

/**
 * The briefing `loft yard read --context` adds for the caller `argv` names, as loft works it out:
 * the working stage is the one stage of the caller's active assigned tasks, else its own active
 * stage, else its session's main agent's.
 */
export function briefingOf(doc: Doc, argv: readonly string[]): Doc {
  const session = flagOf(argv, '--session')
  const agent = flagOf(argv, '--agent')
  const stages = (doc.stages ?? []) as Stage[]
  const tasks = stages.flatMap(each => (each.blocks ?? [])
    .filter(held => held.kind === 'task' && sameActor(held.version.body.assignee, session, agent))
    .map(held => ({ id: held.id, stage: each.id, title: String(held.version.body.title), state: String(held.version.body.state) })))
  const busy = [...new Set(tasks.filter(task => task.state === 'active').map(task => task.stage))]
  const active = (owner: (actor: unknown) => boolean) => stages.find(each => each.version.body.status === 'active' && owner(each.owner))
  const working = busy.length > 1 ? undefined : busy.length === 1 ? stages.find(each => each.id === busy[0])
    : active(owner => sameActor(owner, session, agent)) ?? active(owner => sameActor(owner, session, undefined))
  const assigned = tasks.filter(task => task.state === 'open' || task.state === 'active')
  const lines = [
    `Yard Log of \`${String(doc.branch)}\`.`,
    working ? `Your working stage: \`${working.id}\` — ${String(working.version.body.title)}.` : 'You have no working stage.',
    ...assigned.map(task => `- \`${task.id}\`, ${task.state}, in stage \`${task.stage}\``),
  ]
  return {
    page: PAGE, revision: doc.revision, since: doc.lastRead, noLog: false, closed: false, ignored: doc.ignored,
    stage: working ? { id: working.id, title: working.version.body.title } : null,
    assigned, open: [], responded: [], text: lines.join('\n'),
  }
}

export type Run = { argv: readonly string[]; cwd?: string; env?: Record<string, string>; stdin?: string; spawned: boolean }

const out = (doc: Doc | undefined, exitCode = 0, stderr = '') => ({
  value: { exitCode, stdout: doc === undefined ? '' : JSON.stringify(doc), stderr, isStdoutTruncated: false, isStderrTruncated: false },
})

/** The subcommand of `loft yard` a run names. */
export const subOf = (argv: readonly string[]) => argv[argv.indexOf('yard') + 1]

/** The value after a flag in a run's arguments. */
export const flagOf = (argv: readonly string[], flag: string) => {
  const at = argv.indexOf(flag)
  return at === -1 ? undefined : argv[at + 1]
}

export type World = ReturnType<typeof world>

/** Settles the clock of the world the running test made last, as a session's start does. */
let settle: (() => Promise<unknown>) | undefined

export function world(on: On, options: { protocol?: number | 'none' | 'unrunnable'; serve?: 'serving' | 'running' } = {}) {
  const clock = mock.clock(on, { now: Date.UTC(2026, 9, 22, 9, 30, 0) })
  mock.env(on, { HOME })
  const state = {
    session: SESSION,
    cwd: TREE,
    protocol: options.protocol ?? 1,
    serve: options.serve ?? 'running',
    read: readDoc(),
    /** What each command answers, over the defaults, by subcommand. */
    answers: {} as Record<string, { doc?: Doc; exitCode?: number }>,
    /** What each command prints as it is, over the answers, by subcommand: its standard output and whether Claude Code cut it short. */
    printed: {} as Record<string, { stdout: string; truncated?: boolean; stderr?: string }>,
    runs: [] as Run[],
    /** Optional per-write contract response, used to exercise concurrent task updates. */
    writing: undefined as ((run: Run) => { doc?: Doc; exitCode?: number } | undefined) | undefined,
    /**
     * What each `loft yard wait` gives in turn, or makes when it runs from what the mod wrote before
     * it; once none is left, it waits on `page`.
     */
    waits: [] as (Doc | (() => Doc))[],
    page: deferred<Doc>(),
    serverStops: deferred<void>(),
    posts: [] as { url: string; headers: Record<string, string>; events: Doc[] }[],
    /** The sessions the mod told the page server are live, on its beat. */
    heard: [] as string[],
    /** The address each beat a page server took was posted to. */
    heardAt: [] as string[],
    beats: [] as Doc[],
    /** Whether a page server answers the beat; where none does, the mod runs `loft yard serve`. */
    answersBeat: false,
    /** The one port a page server listens on, where a test names one: a post to another is refused. */
    servedPort: undefined as number | undefined,
    /** The port `loft yard serve` reports for the page server that serves or runs. */
    reportedPort: 7411,
    postStatus: 200,
    /** What the page server's answer to a post waits on. */
    answering: Promise.resolve() as Promise<unknown>,
    /** What each `loft yard read` waits on before it answers. */
    reading: Promise.resolve() as Promise<unknown>,
    /** How many `loft yard wait` runs were killed before they printed. */
    killed: 0,
    toasts: [] as string[],
    tools: [] as { name: string; inputSchema: unknown }[],
    /** The dialogs and tool calls that reached Claude Code's own tools, and how each is answered. */
    calls: [] as Doc[],
    terminal: {} as Record<string, (e: Doc) => Promise<Doc> | Doc>,
    agents: [] as { id: string; type: string }[],
    /** The id of the next subagent Claude Code starts. */
    spawning: 'a9',
    /** Whether a page server has made `~/.yard/token` yet. */
    hasToken: true,
  }

  on('session.id', () => ({ value: state.session }))
  on('session.cwd', () => ({ value: state.cwd }))
  on('session.start', ($, e) => ({ cwd: e.cwd }))
  on('session.end', ($, e) => ({ sessionId: e.sessionId }))
  on('turn.start', ($, e) => ({ turnId: e.turnId }))
  on('turn.complete', () => ({ text: '' }))
  // Beneath the mod, Claude Code starts the subagent the test names and runs it on its model.
  on('agent.spawn', () => ({ model: 'claude-test-1', agentId: state.spawning }))
  on('ui.toast', ($, e) => {
    state.toasts.push(e.text)
    return { value: undefined }
  })
  on('tool.register', ($, e) => {
    state.tools.push({ name: e.name, inputSchema: e.inputSchema })
    return { value: { tool: `mcp__loft__${e.name}` } }
  })
  on('agent.list', () => ({ value: state.agents.map(agent => ({ ...agent, status: 'running' })) as never }))
  on('fs.read', ($, e) => (state.hasToken && e.path === `${HOME}/.yard/token` ? { value: 'token-1\n' } : { deny: `no file ${e.path}` }))
  on('http.fetch', async ($, e) => {
    if (state.servedPort !== undefined && new URL(e.url).port !== String(state.servedPort)) {
      return { deny: `connection refused: ${e.url}` }
    }
    if (e.url.endsWith('/api/heard')) {
      const beat = JSON.parse(String(e.init?.body ?? '{}')) as Doc
      state.heard.push(String(beat.session))
      state.heardAt.push(e.url)
      state.beats.push(beat)
      const status = state.answersBeat ? 200 : 503
      return { value: { status, ok: state.answersBeat, headers: {}, text: '{}' } as never }
    }
    await state.answering
    const headers = (e.init?.headers ?? {}) as Record<string, string>
    state.posts.push({ url: e.url, headers, events: JSON.parse(String(e.init?.body ?? '[]')) as Doc[] })
    const ok = state.postStatus >= 200 && state.postStatus < 300
    return { value: { status: state.postStatus, ok, headers: {}, text: '{"recorded":1}' } as never }
  })

  on('process.run', async ($, e) => {
    state.runs.push({ argv: e.argv, cwd: e.init?.cwd, env: e.init?.env, stdin: e.init?.stdin, spawned: false })
    if (!e.argv.includes('yard')) return out(undefined, 0, '')
    if (state.protocol === 'unrunnable') return { deny: 'loft is not on PATH' }
    const sub = subOf(e.argv)
    if (state.protocol === 'none') return out(undefined, 2, "error: unrecognized subcommand 'yard'")
    const raw = sub ? state.printed[sub] : undefined
    if (raw) return { value: { exitCode: 0, stdout: raw.stdout, stderr: raw.stderr ?? '', isStdoutTruncated: raw.truncated ?? false, isStderrTruncated: false } }
    const given = sub ? state.answers[sub] : undefined
    if (given) return out(given.doc, given.exitCode ?? 0)
    switch (sub) {
      case 'protocol':
        return out({ schema: 'loft-yard-protocol/1', outcome: 'ok', errors: [], protocol: state.protocol, loft: '2.2.1', formats: {} })
      case 'read':
        await state.reading
        if (!e.argv.includes('--context') || 'context' in state.read) return out(state.read)
        return out({ ...state.read, context: briefingOf(state.read, e.argv) })
      case 'task':
        return out({ outcome: e.argv.includes('--release') ? 'released' : 'changed', errors: [], revision: 5 })
      case 'write': {
        const response = state.writing?.({ argv: e.argv, cwd: e.init?.cwd, stdin: e.init?.stdin, spawned: false })
        if (response) return out(response.doc, response.exitCode ?? 0)
        const batch = JSON.parse(e.init?.stdin ?? '[]') as { id: string }[]
        return out({
          outcome: 'written',
          errors: [],
          revision: 5,
          blocks: batch.map(each => ({ id: each.id, key: `k-${each.id}` })),
          ...(e.argv.includes('--send') ? { submission: 'sub-1' } : {}),
        })
      }
      case 'answer':
        return out({ outcome: 'answered', errors: [] })
      default:
        return out({ outcome: sub === 'close' ? 'closed' : `${sub ?? ''}ed`, errors: [] })
    }
  })

  on('process.spawn', async function* ($, e, next) {
    state.runs.push({ argv: e.argv, cwd: e.cwd, env: e.env, spawned: true })
    const sub = subOf(e.argv)
    if (sub === 'serve') {
      yield { stream: 'stdout' as const, text: `${JSON.stringify({ outcome: state.serve, port: state.reportedPort, errors: [] })}\n` }
      if (state.serve === 'serving') await state.serverStops.promise
      return { value: { code: 0, signal: null } }
    }
    // A wait the mod ends is killed: the kit aborts its dispatch.
    const signal = (next as { signal?: AbortSignal }).signal
    const killed = new Promise<null>(done => signal?.addEventListener('abort', () => done(null), { once: true }))
    const given = state.waits.shift()
    const doc = (typeof given === 'function' ? given() : given) ?? (await Promise.race([state.page.promise, killed]))
    if (doc === null) {
      state.killed++
      return { value: { code: null, signal: 'SIGTERM' } }
    }
    yield { stream: 'stdout' as const, text: JSON.stringify(doc) }
    return { value: { code: doc.outcome === 'refused' ? 1 : 0, signal: null } }
  })

  on('tool.call', async ($, e) => {
    state.calls.push(e as Doc)
    const answer = state.terminal[String(e.tool)]
    if (!answer) return { result: { success: true }, text: 'ok' }
    return answer(e as Doc) as never
  })

  settle = () => clock.settle()
  return { clock, state }
}

/** The `loft yard` runs, as the words after `yard`. */
export const yardRuns = (w: World) =>
  w.state.runs.filter(run => run.argv.includes('yard')).map(run => run.argv.slice(run.argv.indexOf('yard') + 1).join(' '))

/** Starts the session, as Claude Code does, and lets what the start set going settle. */
export async function start($: Engine, cwd = TREE) {
  const started = await $.session.start({ cwd, surface: 'terminal', isInteractive: true })
  await settle?.()
  return started
}

/** Starts subagent `id` of type `type`, as the Agent tool does. */
export async function spawn($: Engine, w: World, id: string, type: string) {
  w.state.spawning = id
  return $.agent.spawn({ prompt: 'Go.', subagentType: type, description: 'work' } as never)
}

/** The id Claude Code gave the last call of `tool` that reached its own tool, as a block id holds it. */
export const callIdOf = (w: World, tool: string) => idOf(String(w.state.calls.filter(call => call.tool === tool).at(-1)?.tool_use_id))

/** The blocks of each batch the mod wrote, in order. */
export const writesOf = (w: World) =>
  w.state.runs.filter(run => run.argv.includes('write')).map(run => JSON.parse(run.stdin ?? '[]') as Record<string, unknown>[])

/** The id of the question the mod wrote last. */
export const askedId = (w: World) => String(writesOf(w).at(-1)?.find(each => each.kind === 'question')?.id)
