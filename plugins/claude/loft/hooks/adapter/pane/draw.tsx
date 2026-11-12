// How the Yard pane draws the branch's Yard Log on every surface: a header naming the branch, the
// revision and the Yard page; what waits on the person; then the stages in order, a stage that is
// `done` or `dropped` folded until the person unfolds it (the Feature "Follow Yard Log in a pane").

import type { EngineInterface, On, RenderSurface } from 'claude-code'

import type { PaneBlock, PaneShown, PaneStage, PaneView, YardPane, YardPaneDrafts } from '../../../types'
import { textOf as text } from '../../model'
import type { Given } from '../answers'
import { answerIn, decisionGiven, questionGiven } from './answer'
import { draftsOf, paneOf, setDrafts } from './atoms'
import { PANE } from './follow'
import { head } from './head'
import { bodyOf, type Acts, type Drawing, type Table } from './kit'
import { answeredOn, waitingIn } from './model'
import { paletteOf } from './theme'

/**
 * Whether each surface draws a text field, as the `Elements` it declares say: the mobile app's has
 * no `Input`, though `$.ui.resolve` hands one out that draws nothing. A surface added to `Elements`
 * fails the build until it is named here.
 */
const FIELDED: Record<RenderSurface, boolean> = { terminal: true, desktop: true, vscode: true, mobile: false }

const FOLDED = new Set(['done', 'dropped'])

function options(t: Table, block: PaneBlock, kept: YardPaneDrafts, acts: Acts) {
  const { Box, Text, Button } = t
  const body = bodyOf(block)
  const listed = Array.isArray(body.options) ? (body.options as Record<string, unknown>[]) : []
  const ticked = kept.ticked[block.key] ?? []
  return listed.map((option, n) => {
    const label = text(option.label)
    const named = option.recommended === true ? `${label} (recommended)` : label
    const shown = body.multiple === true ? `${ticked.includes(label) ? '☑' : '☐'} ${named}` : named
    return (
      <Box flexDirection="column">
        <Button key={`option-${block.key}-${n}`} label={shown} onPress={() => acts.choose(block, label)} />
        {text(option.detail) ? <Text dimColor>{text(option.detail)}</Text> : null}
      </Box>
    )
  })
}

/** The field for the person's own words, or a reason, on a surface that has one. */
function field(t: Table, block: PaneBlock, kept: YardPaneDrafts, labels: { placeholder: string; submitLabel: string }, acts: Acts, onSubmit: (words: string) => void) {
  const { Input } = t
  if (!Input) return null
  const prefix = block.kind === 'question' ? 'words' : 'reason'
  return (
    <Input
      key={`${prefix}-${block.key}`}
      placeholder={labels.placeholder}
      submitLabel={labels.submitLabel}
      value={kept.words[block.key] ?? ''}
      onInput={words => acts.type(block, words)}
      onSubmit={onSubmit}
    />
  )
}

/** A block that waits on the person, with what it asks and the controls that answer it. */
function asking(t: Table, block: PaneBlock, kept: YardPaneDrafts, acts: Acts) {
  const { Box, Text, Button, Markdown } = t
  const body = bodyOf(block)
  if (block.kind === 'question') {
    const ownWords = body.ownWords === false ? null : field(t, block, kept, { placeholder: 'Or your own words', submitLabel: 'send' }, acts, words => acts.send(block, words))
    return (
      <Box flexDirection="column" marginTop={1}>
        <Text bold>{text(body.header)}</Text>
        <Text>{text(body.prompt)}</Text>
        {text(body.context) ? <Markdown text={text(body.context)} /> : null}
        {options(t, block, kept, acts)}
        {ownWords}
        {body.multiple === true ? <Button key={`send-${block.key}`} label="Send" variant="primary" onPress={() => acts.send(block)} /> : null}
      </Box>
    )
  }
  return (
    <Box flexDirection="column" marginTop={1}>
      <Text bold>{text(body.title)}</Text>
      <Markdown text={text(body.markdown)} />
      <Box flexDirection="row" gap={1}>
        <Button key={`approve-${block.key}`} label="Approve" variant="primary" onPress={() => acts.decide(block, true)} />
        <Button key={`reject-${block.key}`} label="Reject" onPress={() => acts.decide(block, false)} />
      </Box>
      {field(t, block, kept, { placeholder: 'A reason, to reject', submitLabel: 'reject' }, acts, words => acts.decide(block, false, words))}
    </Box>
  )
}

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

/** One block of a stage, as the stage holds it. */
function held(t: Table, block: PaneBlock) {
  const { Box, Text, Markdown } = t
  const body = bodyOf(block)
  switch (block.kind) {
    case 'task': {
      const fix = body.type === 'fix' ? ' · fix' : ''
      return <Text>{`${text(body.title)} · ${text(body.state)}${fix}`}</Text>
    }
    case 'note':
      return (
        <Box flexDirection="column">
          {text(body.title) ? <Text bold>{text(body.title)}</Text> : null}
          <Markdown text={text(body.markdown)} />
        </Box>
      )
    case 'decision':
      return (
        <Box flexDirection="column">
          <Text bold>{`Decision: ${text(body.title)}`}</Text>
          <Text>{text(body.consequence)}</Text>
        </Box>
      )
    case 'loft':
      return (
        <Box flexDirection="column">
          <Text dimColor>{`Loft ${text(body.view)}`}</Text>
          {text(body.note) ? <Markdown text={text(body.note)} /> : null}
        </Box>
      )
    default: {
      const named = text(body.prompt) || text(body.title)
      const channel = answeredOn(block)
      return <Text dimColor>{`${named} · ${channel ? `answered on ${channel}` : 'waits on you'}`}</Text>
    }
  }
}

function stageOf(t: Table, stage: PaneStage, unfolded: readonly string[], acts: Acts) {
  const { Box, Text, Button } = t
  const body = bodyOf(stage)
  const status = text(body.status)
  const foldable = FOLDED.has(status)
  const isFolded = foldable && !unfolded.includes(stage.key)
  return (
    <Box flexDirection="column" marginTop={1}>
      <Box flexDirection="row" gap={1}>
        <Text bold>{`${text(body.title)} · ${status}`}</Text>
        {foldable ? <Button key={`unfold-${stage.key}`} label={isFolded ? 'Show' : 'Hide'} dimColor onPress={() => acts.toggleFold(stage.key)} /> : null}
      </Box>
      {isFolded ? null : (
        <Box flexDirection="column" paddingLeft={2}>
          {text(body.goal) ? <Text dimColor>{text(body.goal)}</Text> : null}
          {stage.blocks.map(each => held(t, each))}
        </Box>
      )}
    </Box>
  )
}

/** The pane's tree for what it holds. */
function drawn(drawing: Drawing, { view, failure, notice }: YardPane, kept: YardPaneDrafts) {
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
          {view.stages.map(each => stageOf(t, each, kept.unfolded, acts))}
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
        unfolded: before.unfolded.includes(stage) ? before.unfolded.filter(each => each !== stage) : [...before.unfolded, stage],
      })),
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
    return drawn({ t, palette: paletteOf(held.dark), acts: actsOf($) }, held, kept)
  })
}
