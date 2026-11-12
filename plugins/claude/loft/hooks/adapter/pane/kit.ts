// What the Yard pane's drawing shares between its parts: the elements a surface hands it, what a
// press or a submit in it does, and the palette it draws in (the Feature "See Yard Log in the pane
// as on the Yard page").

import type { Elements } from 'claude-code'

import type { PaneBlock, PaneShown } from '../../../types'
import type { Palette } from './theme'

/** The elements every surface draws; `Input` only where the surface has a text field. */
export type Table = Pick<Elements['mobile'], 'Box' | 'Text' | 'Button' | 'Link' | 'Markdown'> & { Input?: Elements['desktop']['Input'] }

/** What a press or a submit in the pane does. */
export type Acts = {
  /** Shows a view, keeping the one shown to go back to once nothing waits. */
  show: (view: PaneShown) => void
  /** Opens a stage's row on the rail, or folds it. */
  toggleFold: (stage: string) => void
  /** Chooses a stage, which the timeline then shows in full. */
  pick: (stage: string) => void
  /** An option pressed: recorded for a question that takes one, ticked or unticked for one that takes more. */
  choose: (block: PaneBlock, label: string) => void
  /** The options ticked and the words typed, sent; or words submitted in the field, with what is ticked. */
  send: (block: PaneBlock, words?: string) => void
  /** The words in a block's field, kept as the person types. */
  type: (block: PaneBlock, words: string) => void
  decide: (block: PaneBlock, isApproved: boolean, reason?: string) => void
}

/** What every part of the drawing is drawn with. */
export type Drawing = { t: Table; palette: Palette; acts: Acts }

export const bodyOf = (block: PaneBlock) => block.version.body
