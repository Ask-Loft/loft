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
    beats: [] as Doc[],
    /** Whether a page server answers the beat; where none does, the mod runs `loft yard serve`. */
    answersBeat: false,
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
    permission: (e: { tool_input: unknown }): unknown => ({}),
    /** Whether a page server has made `~/.yard/token` yet. */
    hasToken: true,
  }

  on('session.id', () => ({ value: state.session }))
  on('session.cwd', () => ({ value: state.cwd }))
  on('session.start', ($, e) => ({ cwd: e.cwd }))
  on('session.end', ($, e) => ({ sessionId: e.sessionId }))
  on('turn.start', ($, e) => ({ turnId: e.turnId }))
  on('turn.complete', () => ({ text: '' }))
  on('session.compact', ($, e) => ({ messages: e.messages, tokensBefore: 9000, tokensAfter: 1200, usage: { input_tokens: 7000, output_tokens: 900, cache_read_input_tokens: 0, cache_creation_input_tokens: 0 } }) as never)
  // Claude Code's own classic hooks: none adds anything.
  on('classic.SessionStart', () => ({}))
  on('classic.SubagentStart', () => ({}))
  on('classic.SubagentStop', () => ({}))
  on('classic.UserPromptSubmit', () => ({}))
  on('classic.PostToolUse', () => ({}))
  on('classic.Stop', () => ({}))
  // Beneath the mod, Claude Code answers a permission request at once with no decision: its
  // dialog shows apart from the request, and the person answers there.
  on('classic.PermissionRequest', ($, e) => state.permission(e as never) as never)
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
    if (e.url.endsWith('/api/heard')) {
      const beat = JSON.parse(String(e.init?.body ?? '{}')) as Doc
      state.heard.push(String(beat.session))
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
        return out(state.read)
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
      yield { stream: 'stdout' as const, text: `${JSON.stringify({ outcome: state.serve, errors: [] })}\n` }
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

  return { clock, state }
}

/** The `loft yard` runs, as the words after `yard`. */
export const yardRuns = (w: World) =>
  w.state.runs.filter(run => run.argv.includes('yard')).map(run => run.argv.slice(run.argv.indexOf('yard') + 1).join(' '))

/** Starts the session: Claude Code's start, then its classic SessionStart. */
export async function start($: Engine, cwd = TREE) {
  await $.session.start({ cwd, surface: 'terminal', isInteractive: true })
  return $.classic.SessionStart({ source: 'startup', cwd })
}

/** The id Claude Code gave the last call of `tool` that reached its own tool, as a block id holds it. */
export const callIdOf = (w: World, tool: string) => idOf(String(w.state.calls.filter(call => call.tool === tool).at(-1)?.tool_use_id))

/** The blocks of each batch the mod wrote, in order. */
export const writesOf = (w: World) =>
  w.state.runs.filter(run => run.argv.includes('write')).map(run => JSON.parse(run.stdin ?? '[]') as Record<string, unknown>[])

/** The id of the question the mod wrote last. */
export const askedId = (w: World) => String(writesOf(w).at(-1)?.find(each => each.kind === 'question')?.id)
