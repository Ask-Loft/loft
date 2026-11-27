// The page server: every BEAT the mod tells it that the session is live; at the session's first
// beat, and after it only where no page server answers, it runs `loft yard serve`, which records
// the session as heard, registers its tree and exits where a page server of a newer version, or of
// its version on the addresses of `yardListen` or, given `--join`, on any, serves the port, and
// otherwise becomes the page server and lives as long as the mod (the Feature "Share one page
// server"). The first beat runs it even where a page server answers, so that one listening on
// other addresses than the setting names is replaced, unless the setting names none: the mod
// cannot tell a setting the person emptied from one its session never loaded, so such a session
// joins whatever page server runs and never narrows it. The mod then reaches the page server on
// the port the command reports, which is not the plugin's `yardPort` where another `loft` started
// it elsewhere. A tree a tool call reaches is registered through `loft yard serve` too.

import type { EngineInterface } from 'claude-code'

import { QUIET, type Settings } from '../loft'
import { parseDocument } from '../model'
import { HOST, LISTENS } from './contract'
import { answerOf, type Outbox } from './outbox'
import { argvOf } from './run'

/** How often the mod tells the page server its session is live, in milliseconds. */
export const BEAT = 5000

/** How long after the session's first beat the mod may run `loft yard serve` without `--join`, in milliseconds. */
export const FIRST_RUN = 30000

/**
 * What the mod knows of the page server: whether its own child serves; whether a beat has run
 * `loft yard serve` yet, the one run that may replace a page server, and how many beats have
 * come since the session's first, which bound when that run may come; the trees the mod
 * registered, as its session spells them; for each tree whose registration the page server
 * answered, the top of its worktree by its real path and as spelled; and the real path of each
 * tree served, as the last answer named them.
 */
export type Server = { isServing: boolean; isBeating: boolean; hasServed: boolean; ticks: number; trees: Set<string>; tops: Map<string, string[]>; served: Set<string> }

export function serverOf(): Server {
  return { isServing: false, isBeating: false, hasServed: false, ticks: 0, trees: new Set(), tops: new Map(), served: new Set() }
}

/**
 * Every tree the mod knows to class a path against, by each spelling it knows. A directory the
 * mod registered stands for its tree only until the page server names the tree's top: a
 * subdirectory's own `spec/` is not the specification.
 */
export function known(server: Server): Set<string> {
  const unanswered = [...server.trees].filter(tree => (server.tops.get(tree) ?? []).length === 0)
  return new Set([...unanswered, ...[...server.tops.values()].flat(), ...server.served])
}

/** The texts an answer's field `name` holds. */
function textsOf(answer: Record<string, unknown>, name: string): string[] {
  const value = answer[name]
  const all = Array.isArray(value) ? value : [value]
  return all.filter((each): each is string => typeof each === 'string')
}

/** Takes the real path of each tree served from an answer of the page server that names them. */
function learn(server: Server, answer: Record<string, unknown>) {
  if (Array.isArray(answer.trees)) server.served = new Set(textsOf(answer, 'trees'))
}

/** The port a `loft yard serve` document reports for the page server that serves or runs, if any. */
function portOf(doc: Record<string, unknown>): number | undefined {
  const { outcome, port } = doc
  if (outcome !== 'serving' && outcome !== 'running') return undefined
  return typeof port === 'number' && Number.isInteger(port) && port > 0 && port < 65536 ? port : undefined
}

/**
 * Runs `loft yard serve` for the session and the tree; resolves with its first document's outcome,
 * and points `box` at the port it reports. Where it says it serves, its loop runs on after this
 * resolves, and ends with the child. Only the session's first beat, where `yardListen` names
 * addresses, replaces a page server of its version on other addresses; every other run joins it,
 * so that two sessions naming different addresses never replace each other in turn.
 */
export function serve($: EngineInterface, settings: Settings, server: Server, box: Outbox, session: string, tree: string, join: boolean) {
  const isListening = (settings.protocol ?? 1) >= LISTENS
  const listen = isListening ? settings.listen.flatMap(address => ['--listen', address]) : []
  const joins = isListening && join ? ['--join'] : []
  const argv = argvOf(settings, ['serve', '--session', session, '--tree', tree, ...listen, ...joins], true)
  const stream = $.process.spawn({ argv, cwd: tree, env: QUIET })
  return new Promise<string | undefined>(settled => {
    let stdout = ''
    let outcome: string | undefined
    let isFirst = true
    void (async () => {
      try {
        for await (const piece of stream) {
          if (piece.stream !== 'stdout' || !isFirst) continue
          stdout += piece.text
          const line = stdout.indexOf('\n')
          const doc = parseDocument(line === -1 ? stdout : stdout.slice(0, line))
          if (!doc) continue
          isFirst = false
          outcome = String(doc.outcome)
          box.port = portOf(doc) ?? box.port
          if (outcome === 'serving') server.isServing = true
          settled(outcome)
        }
      } catch {
        // The child could not start, or the module is unloading.
      } finally {
        if (outcome === 'serving') server.isServing = false
        settled(outcome)
      }
    })()
  })
}

/** Tells the page server that `session` is live, learning the trees it serves; whether it took it. */
async function hear($: EngineInterface, box: Outbox, server: Server, session: string, protocol = 1): Promise<boolean> {
  const answer = await answerOf($, box, '/api/heard', { session, ...(protocol >= 2 ? { host: HOST } : {}) })
  if (answer !== undefined) learn(server, answer)
  return answer !== undefined
}

/**
 * One beat: tells the page server the session is live, and registers the session's tree with one
 * that answers until it takes it, learning the tree's top from the answer; until a beat has run
 * `loft yard serve`, and after that where none answers, runs it, unless the mod's own child serves
 * or the last run has yet to say what it did. The first run joins no page server, so that one on
 * other addresses than `yardListen` names is replaced, even where a beat that hangs on its answer
 * leaves the run to a later beat, unless it comes later than FIRST_RUN after the session's start
 * or `yardListen` names no address. `loft yard serve` reports no top, so a beat that reaches the
 * page server registers the tree once more to learn it.
 */
export async function beat($: EngineInterface, settings: Settings, server: Server, box: Outbox, session: string, tree: string) {
  const isHeard = await hear($, box, server, session, settings.protocol)
  const isBusy = server.isBeating || server.isServing
  if (isHeard && (server.hasServed || isBusy)) return learnTop($, server, box, tree)
  if (isBusy) return
  const join = server.hasServed || server.ticks * BEAT >= FIRST_RUN || settings.listen.length === 0
  server.hasServed = true
  server.isBeating = true
  try {
    const outcome = await serve($, settings, server, box, session, tree, join)
    if (outcome === 'serving' || outcome === 'running') server.trees.add(tree)
  } finally {
    server.isBeating = false
  }
  if (isHeard) await learnTop($, server, box, tree)
}

/** Registers the session's tree with the page server that answers, until it names the tree's top. */
async function learnTop($: EngineInterface, server: Server, box: Outbox, tree: string) {
  if (server.tops.has(tree)) return
  const answer = await answerOf($, box, '/api/register', { tree })
  if (answer === undefined) return
  server.trees.add(tree)
  learn(server, answer)
  // An answer that names no top, as from a page server that wrote no JSON, is asked again.
  const tops = [...textsOf(answer, 'tree'), ...textsOf(answer, 'spelled')]
  if (tops.length > 0) server.tops.set(tree, tops)
}

/** Registers a tree that a tool call reached and the page server does not serve yet. */
export async function register($: EngineInterface, settings: Settings, server: Server, box: Outbox, session: string, tree: string) {
  if (server.trees.has(tree)) return
  server.trees.add(tree)
  await serve($, settings, server, box, session, tree, true)
}
