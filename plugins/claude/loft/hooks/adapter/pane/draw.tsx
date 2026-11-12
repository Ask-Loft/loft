// How the Yard pane draws the branch's Yard Log on every surface, as the Yard page's companion: the
// head, the stage in hand, then what waits on the person, ready to answer (the Feature "Keep the
// Yard page beside the session").

import type { EngineInterface, On, RenderSurface } from 'claude-code'

import type { PaneBlock, YardPane, YardPaneDrafts } from '../../../types'
import type { Given } from '../answers'
import { answerIn, decisionGiven, questionGiven } from './answer'
import { browserOf, draftsOf, paneOf, setDrafts } from './atoms'
import { PANE } from './follow'
import { head } from './head'
import { bodyOf, type Acts, type Drawing, type Table } from './kit'
import { waitingIn } from './model'
import { PALETTE } from './theme'
import { stageInHand } from './stage'
import { whatWaits } from './waiting'

/**
 * Whether each surface draws a text field, as the `Elements` it declares say: the mobile app's has
 * no `Input`, though `$.ui.resolve` hands one out that draws nothing. A surface added to `Elements`
 * fails the build until it is named here.
 */
const FIELDED: Record<RenderSurface, boolean> = { terminal: true, desktop: true, vscode: true, mobile: false }

function drawn(drawing: Drawing, { view, failure, notice }: YardPane, kept: YardPaneDrafts, isUnreached: boolean) {
  const { Box, Text } = drawing.t
  const failed = failure ? <Text color={drawing.palette.danger}>{failure}</Text> : null
  const noticed = notice ? <Text color={drawing.palette.wait}>{notice}</Text> : null
  if (view === null) return <Box flexDirection="column">{failed ?? <Text dimColor>Reading Yard Log…</Text>}</Box>
  if (view.log === null) {
    return (
      <Box flexDirection="column">
        <Text>{`No Yard Log on ${view.branch} yet.`}</Text>
        {failed}
      </Box>
    )
  }
  const waits = waitingIn(view)
  return (
    <Box flexDirection="column">
      {head(drawing, view, waits.length, isUnreached)}
      {failed}
      {noticed}
      {stageInHand(drawing, view)}
      {waits.length > 0 ? whatWaits(drawing, waits, kept) : null}
    </Box>
  )
}

/**
 * What a press in the pane does, for this session. What the person set and not sent is read when
 * they press, not as it was drawn, so that a tick or a keystroke not yet drawn is not lost.
 */
function actsOf($: EngineInterface): Acts {
  const recorded = (given: Given | undefined) => void (given && answerIn($, given))
  const settled = async (block: PaneBlock) => {
    const kept = await draftsOf($)
    return { ticked: kept.ticked[block.key] ?? [], words: kept.words[block.key] }
  }
  return {
    choose: (block, label) => {
      if (bodyOf(block).multiple !== true) {
        void settled(block).then(now => recorded(questionGiven(block.key, [label], now.words)))
        return
      }
      void setDrafts($, before => {
        const was = before.ticked[block.key] ?? []
        const now = was.includes(label) ? was.filter(each => each !== label) : [...was, label]
        return { ...before, ticked: { ...before.ticked, [block.key]: now } }
      })
    },
    send: (block, typed) =>
      void settled(block).then(now => recorded(questionGiven(block.key, bodyOf(block).multiple === true ? now.ticked : [], typed ?? now.words))),
    type: (block, typed) => void setDrafts($, before => ({ ...before, words: { ...before.words, [block.key]: typed } })),
    decide: (block, isApproved, typed) => void settled(block).then(now => recorded(decisionGiven(block.key, isApproved, typed ?? now.words))),
  }
}

/** Draws the pane on whichever surface asks. */
export function registerDraw(on: On) {
  on('ui.render', { component: 'Pane', requestId: PANE }, async ($, e) => {
    const resolved = $.ui.resolve(e) as unknown as Table
    const t: Table = FIELDED[e.surface] ? resolved : { ...resolved, Input: undefined }
    const [held, kept, browser] = await Promise.all([paneOf($), draftsOf($), browserOf($)])
    return drawn({ t, palette: PALETTE, acts: actsOf($) }, held, kept, e.surface === 'desktop' && browser?.isReached === false)
  })
}
