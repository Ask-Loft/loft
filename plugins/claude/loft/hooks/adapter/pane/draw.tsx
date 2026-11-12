// How the Yard pane draws the branch's Yard Log on every surface: a header naming the branch, the
// revision and the Yard page; what waits on the person; then the stages in order, a stage that is
// `done` or `dropped` folded until the person unfolds it (the Feature "Follow Yard Log in a pane").

import type { EngineInterface, On, RenderSurface } from 'claude-code'

import type { PaneBlock, PaneShown, PaneView, YardPane, YardPaneDrafts } from '../../../types'
import type { Given } from '../answers'
import { answerIn, decisionGiven, questionGiven } from './answer'
import { draftsOf, paneOf, setDrafts } from './atoms'
import { PANE } from './follow'
import { shownIn } from './atoms'
import { head } from './head'
import { bodyOf, type Acts, type Drawing, type Table } from './kit'
import { waitingIn } from './model'
import { PALETTE } from './theme'
import { chart } from './chart'
import { timeline } from './timeline'
import { whatWaits } from './waiting'

/**
 * Whether each surface draws a text field, as the `Elements` it declares say: the mobile app's has
 * no `Input`, though `$.ui.resolve` hands one out that draws nothing. A surface added to `Elements`
 * fails the build until it is named here.
 */
const FIELDED: Record<RenderSurface, boolean> = { terminal: true, desktop: true, vscode: true, mobile: false }

/** Whether each surface draws SVG, as the `Elements` it declares say: all but the terminal. */
const PICTURED: Record<RenderSurface, boolean> = { terminal: false, desktop: true, vscode: true, mobile: true }


function drawn(drawing: Drawing, { view, failure, notice }: YardPane, kept: YardPaneDrafts, columns: number, now: number) {
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
      {shown === 'waiting' ? (
        <Box key="view-waiting" flexDirection="column" marginTop={1}>
          {whatWaits(drawing, view, kept)}
        </Box>
      ) : shown === 'gantt' ? (
        <Box key="view-gantt" flexDirection="column" marginTop={1}>
          {chart(drawing, view, now, columns)}
        </Box>
      ) : (
        <Box key="view-timeline" flexDirection="column">
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
    show: view => void setDrafts($, before => shownIn(before, view, null)),
    answerAt: block =>
      setDrafts($, before => shownIn(before, 'waiting', block.key)).then(() =>
        $.ui.scroll({ to: { key: `ask-${block.key}` }, in: PANE, block: 'start' }).catch(() => undefined),
      ),
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
    const t: Table = { ...resolved, Input: FIELDED[e.surface] ? resolved.Input : undefined, Svg: PICTURED[e.surface] ? resolved.Svg : undefined }
    const [held, kept, now] = await Promise.all([paneOf($), draftsOf($), $.clock.now()])
    return drawn({ t, palette: PALETTE, acts: actsOf($) }, held, kept, e.props.bodyColumns, now)
  })
}
