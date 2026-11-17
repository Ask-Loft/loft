// The events not yet posted to the page server: kept in order, at most MOST, and posted with the
// page server's control token once it answers (the Feature "Record a session's events").

import type { EngineInterface } from 'claude-code'

import { isObject } from '../model'

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
  trim(box)
}

function trim(box: Outbox) {
  const over = box.queue.length - MOST
  if (over <= 0) return
  box.queue.splice(0, over)
  box.dropped += over
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

/**
 * Posts `body` to the page server's `path` with the control token; its answer where it took it, an
 * empty one where the answer is no JSON object, and none where it did not take it. A token it
 * refuses is forgotten, so that the next post reads the one a newer page server made.
 */
export async function answerOf($: EngineInterface, box: Outbox, path: string, body: unknown): Promise<Record<string, unknown> | undefined> {
  const token = await tokenOf($, box)
  if (token === undefined) return undefined
  try {
    const posted = await $.http.fetch(`http://127.0.0.1:${box.port}${path}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify(body),
    })
    if (posted.status === 401 || posted.status === 403) box.token = undefined
    if (!posted.ok) return undefined
    return objectOf(posted.text)
  } catch {
    return undefined
  }
}

/** `text` as a JSON object, or an empty one. */
function objectOf(text: string): Record<string, unknown> {
  try {
    const doc: unknown = JSON.parse(text)
    return isObject(doc) ? doc : {}
  } catch {
    return {}
  }
}

/** Posts `body` to the page server's `path` with the control token; whether it took it. */
export async function postToServer($: EngineInterface, box: Outbox, path: string, body: unknown): Promise<boolean> {
  return (await answerOf($, box, path, body)) !== undefined
}

/**
 * Posts every kept event in order; those the page server does not take stay kept. A post takes its
 * events out of the queue while it is under way, so an event kept meanwhile waits behind them.
 */
export async function flush($: EngineInterface, box: Outbox): Promise<void> {
  if (box.isPosting) return
  box.isPosting = true
  try {
    while (box.queue.length > 0) {
      if ((await tokenOf($, box)) === undefined) return
      const sent = box.queue.splice(0)
      const dropped = box.dropped
      // The first event posted after a drop says how many were dropped.
      const body = sent.map((event, n) => (n === 0 && dropped > 0 ? { ...event, dropped } : event))
      if (!(await postToServer($, box, '/api/events', body))) {
        box.queue.unshift(...sent)
        trim(box)
        return
      }
      box.dropped -= dropped
    }
  } finally {
    box.isPosting = false
  }
}
