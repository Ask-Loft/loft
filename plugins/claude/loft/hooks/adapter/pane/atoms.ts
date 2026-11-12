// What the Yard pane keeps for the session in `$.state`: its view of Yard Log, and what the person
// has set in it and not sent. Every read and write of them is here, beside the atoms, where Claude
// Code's scan of the module lists them. Change a tag when its value's fields change: a reload then
// declines what an earlier version kept.

import { atom, read, update } from 'claude-code'
import type { EngineInterface } from 'claude-code'

import type { PaneShown, YardPane, YardPaneDrafts } from '../../../types'

const pane = atom({ plugin: 'loft', key: 'yardPane' } as const, { view: null, failure: null, notice: null, dark: false } as YardPane, { shape: 'yard-pane-6' })

const drafts = atom({ plugin: 'loft', key: 'yardPaneDrafts' } as const, { shown: 'timeline', previous: 'timeline', at: null, seen: [], chosen: null, toggled: [], ticked: {}, words: {} } as YardPaneDrafts, { shape: 'yard-pane-drafts-4' })

/** The pane's view and its last failure; read while drawing, it redraws the pane on a change. */
export function paneOf($: EngineInterface) {
  return read($, pane)
}

export function setPane($: EngineInterface, next: (before: YardPane) => YardPane) {
  return update($, pane, next)
}

/** What the person has set in the pane and not sent. */
export function draftsOf($: EngineInterface) {
  return read($, drafts)
}

export function setDrafts($: EngineInterface, next: (before: YardPaneDrafts) => YardPaneDrafts) {
  return update($, drafts, next)
}

/**
 * The drafts with `view` shown, at the block `at`: the view shown before what waits is kept, to
 * show again once nothing waits.
 */
export function shownIn(before: YardPaneDrafts, view: PaneShown, at: string | null): YardPaneDrafts {
  const previous = view === 'waiting' ? (before.shown === 'waiting' ? before.previous : before.shown) : view
  return { ...before, shown: view, previous, at }
}
