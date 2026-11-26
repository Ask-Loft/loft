// The flush of the outbox in the background (the Feature "Flush in the background"): where the
// `loft` the mod runs speaks the yard protocol that brings `loft yard flush`, the mod runs it
// without waiting for it after each write of Yard Log it makes, at the session's first beat
// (`startFlush`), and at each later beat while the last flush left work (`flushAtBeat`). One flush runs at a time: a write made while
// one runs is followed by one more once it ends. A flush that ends `off` ends them for the rest
// of the session, and one that still runs 6 minutes after it started is stopped.

import type { EngineInterface } from 'claude-code'

import type { Settings } from '../loft'
import { FLUSHES } from './contract'
import { yard } from './run'

/** How long a flush may run before the mod stops it: a minute past the 5 after which it ends by itself. */
export const MOST_MS = 6 * 60_000

/** The `loft yard` commands that write Yard Log, after which the mod flushes. */
export const WRITES: ReadonlySet<string> = new Set(['write', 'task', 'published', 'answer', 'withdraw', 'close'])

export type Flushing = {
  /** Whether a flush the mod started still runs. */
  running: boolean
  /** Whether a write came while it ran, so that one more follows it. */
  again: boolean
  /** Whether the last flush left work: it ended `failed`, `busy` or `paused`, or counted entries `remaining`. */
  leftWork: boolean
  /** Whether a flush ended `off`, which ends them for the session. */
  off: boolean
}

export function flushingOf(): Flushing {
  return { running: false, again: false, leftWork: false, off: false }
}

/** Starts `loft yard flush` in `cwd` without waiting for it, unless none may run. */
export function startFlush($: EngineInterface, settings: Settings, flushing: Flushing, cwd: string): void {
  if (flushing.off || (settings.protocol ?? 0) < FLUSHES) return
  if (flushing.running) {
    flushing.again = true
    return
  }
  flushing.running = true
  void (async () => {
    const ran = await yard($, settings, cwd, ['flush'], { beyond: false, timeoutMs: MOST_MS })
    const outcome = ran.doc?.outcome
    const remaining = ran.doc?.remaining
    flushing.off = outcome === 'off'
    flushing.leftWork = !['sent', 'empty', 'off'].includes(String(outcome))
      || (typeof remaining === 'number' && remaining > 0)
    flushing.running = false
    if (flushing.again) {
      flushing.again = false
      startFlush($, settings, flushing, cwd)
    }
  })()
}

/** At a beat after the session's first: flushes while the last flush left work. */
export function flushAtBeat($: EngineInterface, settings: Settings, flushing: Flushing, cwd: string): void {
  if (flushing.leftWork) startFlush($, settings, flushing, cwd)
}
