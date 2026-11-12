// The Yard pane's command and its follow of Yard Log: `/yard-pane` opens the pane, which reads the
// Yard Log of the session's working tree whole, then every second only what changed since the
// revision it shows, while a surface draws it, and stops once the pane is closed. It never marks a
// revision read (the Feature "Follow Yard Log in a pane").

import type { EngineInterface, On, Timer } from 'claude-code'

import { callerArgs, failureOf, succeeded } from '../run'
import { callerOf, run } from '../state'
import { paneOf, setPane } from './atoms'
import { followed } from './model'

/** The pane's id, which its `ui.render` hook names as `requestId`. */
export const PANE = 'yard'
const COMMAND = 'yard-pane'
/** How often an open pane reads what changed. */
const FOLLOW = 1000

/**
 * What the module keeps of the follow: its timer; whether a read is under way and whether another
 * is asked for after it; the epoch, which an open moves on so that a read begun before it writes
 * nothing; and the notice an answer left for the next write.
 */
const following = {
  timer: undefined as Timer | undefined,
  isReading: false,
  again: false,
  epoch: 0,
  notice: undefined as string | null | undefined,
}

/** Offers `/yard-pane` for the session, to run even while a turn runs. */
export async function registerCommand($: EngineInterface) {
  await $.command.register({ name: COMMAND, description: "Follow the branch's Yard Log in a pane, and answer there", immediate: true })
}

/** Opens the pane, or shows it again, read whole, and follows Yard Log while it stays open. */
async function openPane($: EngineInterface) {
  following.epoch++
  await setPane($, () => ({ view: null, failure: null, notice: null }))
  const opened = await $.ui.open({ id: PANE, title: 'Yard' })
  await readPane($)
  following.timer ??= $.clock.every(FOLLOW, () => void tick($))
  return opened
}

async function tick($: EngineInterface) {
  const open = (await $.ui.panes()).find(each => each.id === PANE)
  if (!open) {
    following.timer?.cancel()
    following.timer = undefined
    return
  }
  if (open.isPlaced && open.isShown) await readPane($)
}

/**
 * Reads what changed since the revision shown, and keeps `notice`, where an answer gives one, for
 * the same write. A read asked for while one is under way runs once that one ends.
 */
export async function readPane($: EngineInterface, notice?: string | null) {
  if (notice !== undefined) following.notice = notice
  following.again = true
  if (following.isReading) return
  following.isReading = true
  try {
    while (following.again) {
      following.again = false
      await readOnce($)
    }
  } finally {
    following.isReading = false
  }
}

/**
 * One read: since the revision shown, or whole where nothing is shown yet or the read names another
 * tree, branch or Yard Log than the one shown. A read that changes nothing writes nothing, and one
 * an open overtook is read again.
 */
async function readOnce($: EngineInterface) {
  const epoch = following.epoch
  const held = await paneOf($)
  const shown = held.view ?? undefined
  let ran = await readSince($, shown?.revision)
  let view = succeeded(ran) ? followed(shown, ran.doc) : shown
  if (view === undefined && shown !== undefined) {
    // Another Yard Log than the one shown: nothing of the one shown stays, read or not.
    ran = await readSince($, undefined)
    view = succeeded(ran) ? followed(undefined, ran.doc) : undefined
  }
  if (epoch !== following.epoch) {
    following.again = true
    return
  }
  const failure = succeeded(ran) ? null : failureOf(ran)
  const notice = following.notice
  following.notice = undefined
  if (view === shown && failure === held.failure && notice === undefined) return
  await setPane($, before => ({ view: view ?? null, failure, notice: notice === undefined ? before.notice : notice }))
}

/** One `loft yard read` for the session's main agent, since `since` where given. */
async function readSince($: EngineInterface, since: number | undefined) {
  const [caller, cwd] = await Promise.all([callerOf($, undefined), $.session.cwd()])
  return run($, cwd, ['read', ...callerArgs(caller), ...(since === undefined ? [] : ['--since', String(since)])])
}

/** Hooks `/yard-pane`, and starts the follow over for a module just loaded. */
export function registerPane(on: On) {
  following.timer?.cancel()
  Object.assign(following, { timer: undefined, isReading: false, again: false, epoch: 0, notice: undefined })
  on('command.run', { command: COMMAND }, async $ => {
    const opened = await openPane($)
    return { text: opened.isPlaced ? "Yard Log's pane is open." : "Yard Log's pane opens once there is room for it." }
  })
}
