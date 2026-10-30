// The page server: every BEAT the mod tells it that the session is live; only where no page server
// answers does it run `loft yard serve`, which records the session as heard, registers its tree
// and exits where a page server of its version or a newer one serves the port, and where none
// does becomes the page server and lives as long as the mod (the Feature "Share one page
// server"). The mod then reaches the page server on the port the command reports, which is not
// the plugin's `yardPort` where another `loft` started it elsewhere. A tree a tool call reaches is
// registered through `loft yard serve` too.

import type { EngineInterface } from 'claude-code'

import { QUIET, type Settings } from '../loft'
import { parseDocument } from '../model'
import { HOST } from './contract'
import { postToServer, type Outbox } from './outbox'
import { argvOf } from './run'

/** How often the mod tells the page server its session is live, in milliseconds. */
export const BEAT = 5000

/** What the mod knows of the page server: whether its own child serves, and the trees registered. */
export type Server = { isServing: boolean; isBeating: boolean; trees: Set<string> }

export function serverOf(): Server {
  return { isServing: false, isBeating: false, trees: new Set() }
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
 * resolves, and ends with the child.
 */
export function serve($: EngineInterface, settings: Settings, server: Server, box: Outbox, session: string, tree: string) {
  const argv = argvOf(settings, ['serve', '--session', session, '--tree', tree], true)
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

/** Tells the page server that `session` is live; whether a page server took it. */
export async function hear($: EngineInterface, box: Outbox, session: string, protocol = 1): Promise<boolean> {
  return postToServer($, box, '/api/heard', { session, ...(protocol >= 2 ? { host: HOST } : {}) })
}

/**
 * One beat: tells the page server the session is live, and registers the session's tree with one
 * that answers until it takes it; where none answers, runs `loft yard serve`, which registers the
 * tree itself, unless the mod's own child serves or the last run has yet to say what it did.
 */
export async function beat($: EngineInterface, settings: Settings, server: Server, box: Outbox, session: string, tree: string) {
  if (await hear($, box, session, settings.protocol)) {
    if (!server.trees.has(tree) && (await postToServer($, box, '/api/register', { tree }))) server.trees.add(tree)
    return
  }
  if (server.isBeating || server.isServing) return
  server.isBeating = true
  try {
    const outcome = await serve($, settings, server, box, session, tree)
    if (outcome === 'serving' || outcome === 'running') server.trees.add(tree)
  } finally {
    server.isBeating = false
  }
}

/** Registers a tree that a tool call reached and the page server does not serve yet. */
export async function register($: EngineInterface, settings: Settings, server: Server, box: Outbox, session: string, tree: string) {
  if (server.trees.has(tree)) return
  server.trees.add(tree)
  await serve($, settings, server, box, session, tree)
}
