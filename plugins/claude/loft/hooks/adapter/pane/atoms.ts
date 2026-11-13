// What the Yard pane keeps for the session in `$.state`: its view of Yard Log, Desktop's browser pane
// as `/yard-pane` left it, and what the person has set in the pane and not sent. Every read and write of them is here, beside the atoms, where Claude
// Code's scan of the module lists them. Change a tag when its value's fields change: a reload then
// declines what an earlier version kept.

import { atom, read, update } from 'claude-code'
import type { EngineInterface } from 'claude-code'

import type { PaneBrowser, YardPane, YardPaneDrafts } from '../../../types'

const pane = atom({ plugin: 'loft', key: 'yardPane' } as const, { view: null, failure: null, notice: null } as YardPane, { shape: 'yard-pane-10' })

const browser = atom({ plugin: 'loft', key: 'yardPaneBrowser' } as const, null as PaneBrowser | null, { shape: 'yard-pane-browser-1' })

const drafts = atom({ plugin: 'loft', key: 'yardPaneDrafts' } as const, { ticked: {}, words: {} } as YardPaneDrafts, { shape: 'yard-pane-drafts-5' })

/** The pane's view and its last failure; read while drawing, it redraws the pane on a change. */
export function paneOf($: EngineInterface) {
  return read($, pane)
}

export function setPane($: EngineInterface, next: (before: YardPane) => YardPane) {
  return update($, pane, next)
}

/** Desktop's browser pane, as `/yard-pane` last left it; read while drawing, it redraws the pane on a change. */
export function browserOf($: EngineInterface) {
  return read($, browser)
}

/** Keeps what `/yard-pane` found of Desktop's browser pane, where it changed. */
export function setBrowser($: EngineInterface, next: PaneBrowser) {
  return update($, browser, before => (before?.tab === next.tab && before?.isReached === next.isReached ? before : next))
}

/** What the person has set in the pane and not sent. */
export function draftsOf($: EngineInterface) {
  return read($, drafts)
}

export function setDrafts($: EngineInterface, next: (before: YardPaneDrafts) => YardPaneDrafts) {
  return update($, drafts, next)
}
