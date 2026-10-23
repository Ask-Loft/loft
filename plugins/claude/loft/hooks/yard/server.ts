// The page server: every BEAT the mod tells it that the session is live; only where no page server
// answers does it run `loft yard serve`, which records the session as heard, registers its tree
// and exits where a page server of its version or a newer one serves the port, and where none
// does becomes the page server and lives as long as the mod (the Feature "Share one page
// server"). A tree a tool call reaches is registered through `loft yard serve` too.

import type { EngineInterface } from 'claude-code'

import { QUIET, type Settings } from '../loft'
import { parseDocument } from '../model'
import { tokenOf, type Outbox } from './outbox'
import { argvOf } from './run'

/** How often the mod tells the page server its session is live, in milliseconds. */
export const BEAT = 5000

/** What the mod knows of the page server: whether its own child serves, and the trees registered. */
export type Server = { isServing: boolean; isBeating: boolean; trees: Set<string> }

export function serverOf(): Server {
  return { isServing: false, isBeating: false, trees: new Set() }
}

/**
 * Runs `loft yard serve` for the session and the tree; resolves with its first document. Where it
 * says it serves, its loop runs on after this resolves, and ends with the child.
 */
export function serve($: EngineInterface, settings: Settings, server: Server, session: string, tree: string) {
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
export async function hear($: EngineInterface, box: Outbox, session: string): Promise<boolean> {
  const token = await tokenOf($, box)
  if (token === undefined) return false
  try {
    const heard = await $.http.fetch(`http://127.0.0.1:${box.port}/api/heard`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ session }),
    })
    if (heard.status === 401 || heard.status === 403) box.token = undefined
    return heard.ok
  } catch {
    return false
  }
}

/**
 * One beat: tells the page server the session is live; where none answers, runs `loft yard
 * serve`, unless the mod's own child serves or the last run has yet to say what it did.
 */
export async function beat($: EngineInterface, settings: Settings, server: Server, box: Outbox, session: string, tree: string) {
  if (await hear($, box, session)) return
  if (server.isBeating || server.isServing) return
  server.isBeating = true
  try {
    await serve($, settings, server, session, tree)
  } finally {
    server.isBeating = false
  }
}

/** Registers a tree that a tool call reached and the page server does not serve yet. */
export async function register($: EngineInterface, settings: Settings, server: Server, session: string, tree: string) {
  if (server.trees.has(tree)) return
  server.trees.add(tree)
  await serve($, settings, server, session, tree)
}
