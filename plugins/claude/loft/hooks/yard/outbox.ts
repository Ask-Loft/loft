// The events not yet posted to the page server: kept in order, at most MOST, and posted with the
// page server's control token once it answers (the Feature "Record a session's events").

import type { EngineInterface } from 'claude-code'

/** The most events kept while the page server does not answer; past it the oldest is dropped. */
export const MOST = 1000

export type Event = Record<string, unknown> & { kind: string }

/** The events not yet posted, and how many were dropped since the last post. */
export type Outbox = {
  port: number
  queue: Event[]
  dropped: number
  isPosting: boolean
  token?: string
  /** Where `~/.yard/token` is, once the home directory is known. */
  tokenPath?: string
}

export function outboxOf(port: number): Outbox {
  return { port, queue: [], dropped: 0, isPosting: false }
}

/** An Act's time: UTC, to the second. */
export function timeOf(ms: number): string {
  return new Date(ms).toISOString().replace(/\.\d+Z$/, 'Z')
}

/** Keeps an event, dropping the oldest past MOST. */
export function keep(box: Outbox, event: Event) {
  box.queue.push(event)
  if (box.queue.length > MOST) {
    box.queue.shift()
    box.dropped++
  }
}

export async function tokenOf($: EngineInterface, box: Outbox): Promise<string | undefined> {
  if (box.token !== undefined || box.tokenPath === undefined) return box.token
  try {
    const token = (await $.fs.read(box.tokenPath)).trim()
    if (token !== '') box.token = token
  } catch {
    // No page server has made the token yet.
  }
  return box.token
}

/** Posts every kept event in order; those the page server does not take stay kept. */
export async function flush($: EngineInterface, box: Outbox): Promise<void> {
  if (box.isPosting) return
  box.isPosting = true
  try {
    while (box.queue.length > 0) {
      const token = await tokenOf($, box)
      if (token === undefined) return
      const sent = box.queue.slice()
      const dropped = box.dropped
      // The first event posted after a drop says how many were dropped.
      const body = sent.map((event, n) => (n === 0 && dropped > 0 ? { ...event, dropped } : event))
      let isTaken = false
      try {
        const posted = await $.http.fetch(`http://127.0.0.1:${box.port}/api/events`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
          body: JSON.stringify(body),
        })
        isTaken = posted.ok
        if (posted.status === 401 || posted.status === 403) box.token = undefined
      } catch {
        isTaken = false
      }
      if (!isTaken) return
      box.queue.splice(0, sent.length)
      box.dropped -= dropped
    }
  } finally {
    box.isPosting = false
  }
}
