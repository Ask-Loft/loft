// How the Yard pane draws the branch's Yard Log on every surface: a header naming the branch, the
// revision and the Yard page; what waits on the person; then the stages in order, a stage that is
// `done` or `dropped` folded until the person unfolds it (the Feature "Follow Yard Log in a pane").

import type { EngineInterface, On, RenderSurface } from 'claude-code'

import type { PaneBlock, PaneShown, PaneView, YardPane, YardPaneDrafts } from '../../../types'
import type { Given } from '../answers'
import { answerIn, decisionGiven, questionGiven } from './answer'
import { draftsOf, paneOf, setDrafts } from './atoms'
import { PANE } from './follow'
import { asking } from './cards'
import { head } from './head'
import { bodyOf, type Acts, type Drawing, type Table } from './kit'
import { waitingIn } from './model'
import { paletteOf } from './theme'
import { timeline } from './timeline'

/**
 * Whether each surface draws a text field, as the `Elements` it declares say: the mobile app's has
 * no `Input`, though `$.ui.resolve` hands one out that draws nothing. A surface added to `Elements`
 * fails the build until it is named here.
 */
const FIELDED: Record<RenderSurface, boolean> = { terminal: true, desktop: true, vscode: true, mobile: false }


function waiting(t: Table, view: PaneView, kept: YardPaneDrafts, acts: Acts) {
  const { Box, Text } = t
  const blocks = waitingIn(view)
  if (blocks.length === 0) return null
  return (
    <Box key="waiting" flexDirection="column" marginTop={1}>
      <Text bold color="yellow">Waits on you</Text>
      {blocks.map(each => asking(t, each, kept, acts))}
    </Box>
  )
}

function drawn(drawing: Drawing, { view, failure, notice }: YardPane, kept: YardPaneDrafts, columns: number) {
  const { t, acts } = drawing
  const { Box, Text } = t
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
  const shown: PaneShown = kept.shown === 'waiting' && waits.length === 0 ? kept.previous : kept.shown
  return (
    <Box flexDirection="column">
      {head(drawing, view, waits.length, shown)}
      {failed}
      {noticed}
      {shown === 'gantt' ? (
        <Box key="view-gantt" flexDirection="column" marginTop={1}>
          <Text dimColor>Gantt chart</Text>
        </Box>
      ) : (
        <Box key="view-timeline" flexDirection="column">
          {waiting(t, view, kept, acts)}
          {timeline(drawing, view, kept, new Set(waits.map(each => each.key)), columns)}
        </Box>
      )}
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
    show: view =>
      void setDrafts($, before => ({ ...before, shown: view, previous: view === 'waiting' ? (before.shown === 'waiting' ? before.previous : before.shown) : view })),
    toggleFold: stage =>
      void setDrafts($, before => ({
        ...before,
        toggled: before.toggled.includes(stage) ? before.toggled.filter(each => each !== stage) : [...before.toggled, stage],
      })),
    pick: stage => void setDrafts($, before => ({ ...before, chosen: stage, toggled: [], shown: 'timeline' })),
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
    const [held, kept] = await Promise.all([paneOf($), draftsOf($)])
    return drawn({ t, palette: paletteOf(held.dark), acts: actsOf($) }, held, kept, e.props.bodyColumns)
  })
}
