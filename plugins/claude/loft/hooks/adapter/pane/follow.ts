// The Yard pane's command and its follow of Yard Log: `/yard-pane` opens the pane, which reads the
// Yard Log of the session's working tree whole, then every second only what changed since the
// revision it shows, and stops once the pane is closed. It never marks a revision read (the
// Feature "Follow Yard Log in a pane").

import type { EngineInterface, On, Timer } from 'claude-code'

import { callerArgs, failureOf, succeeded } from '../run'
import { continued, run } from '../state'
import { paneOf, setPane } from './atoms'
import { followed, sinceOf } from './model'

/** The pane's id, which its `ui.render` hook names as `requestId`. */
export const PANE = 'yard'
export const COMMAND = 'yard-pane'
/** How often an open pane reads what changed. */
export const FOLLOW = 1000

/** What the module keeps of the follow: its timer, and whether a read is under way. */
const following: { timer: Timer | undefined; isReading: boolean } = { timer: undefined, isReading: false }

/** Offers `/yard-pane` for the session, to run even while a turn runs. */
export async function registerCommand($: EngineInterface) {
  await $.command.register({ name: COMMAND, description: "Follow the branch's Yard Log in a pane, and answer there", immediate: true })
}

/** Opens the pane, or shows it again, read whole, and follows Yard Log while it stays open. */
export async function openPane($: EngineInterface) {
  await setPane($, () => ({ view: null, failure: null, notice: null }))
  const opened = await $.ui.open({ id: PANE, title: 'Yard' })
  await readPane($)
  following.timer ??= $.clock.every(FOLLOW, () => void tick($))
  return opened
}

async function tick($: EngineInterface) {
  const isOpen = (await $.ui.panes()).some(each => each.id === PANE)
  if (!isOpen) {
    following.timer?.cancel()
    following.timer = undefined
    return
  }
  await readPane($)
}

/**
 * Reads what changed since the revision shown, or Yard Log whole where nothing is shown yet or
 * the read names another tree, branch or Yard Log than the one shown.
 */
export async function readPane($: EngineInterface) {
  if (following.isReading) return
  following.isReading = true
  try {
    const shown = (await paneOf($)).view ?? undefined
    let ran = await readSince($, sinceOf(shown))
    let view = succeeded(ran) ? followed(shown, ran.doc) : undefined
    if (succeeded(ran) && view === undefined) {
      ran = await readSince($, undefined)
      view = succeeded(ran) ? followed(undefined, ran.doc) : undefined
    }
    const failure = succeeded(ran) ? null : failureOf(ran)
    await setPane($, before => ({ ...before, view: view ?? before.view, failure }))
  } finally {
    following.isReading = false
  }
}

/** One `loft yard read` for the session's main agent, since `since` where given. */
async function readSince($: EngineInterface, since: number | undefined) {
  const session = await continued($)
  const cwd = await $.session.cwd()
  return run($, cwd, ['read', ...callerArgs({ session }), ...(since === undefined ? [] : ['--since', String(since)])])
}

/** Hooks `/yard-pane`, and starts the follow over for a module just loaded. */
export function registerPane(on: On) {
  following.timer = undefined
  following.isReading = false
  on('command.run', { command: COMMAND }, async $ => {
    const opened = await openPane($)
    return { text: opened.isPlaced ? "Yard Log's pane is open." : "Yard Log's pane opens once there is room for it." }
  })
}
