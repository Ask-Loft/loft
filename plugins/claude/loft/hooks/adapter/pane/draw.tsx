// How the Yard pane draws the branch's Yard Log on every surface: a header naming the branch, the
// revision and the Yard page; what waits on the person; then the stages in order, a stage that is
// `done` or `dropped` folded until the person unfolds it (the Feature "Follow Yard Log in a pane").

import type { EngineInterface, Elements, On } from 'claude-code'

import type { PaneBlock, PaneStage, PaneView, YardPaneDrafts } from '../../../types'
import { draftsOf, paneOf, setDrafts } from './atoms'
import { PANE } from './follow'
import { answeredOn, linkOf, waitingOf } from './model'

/** The elements every surface draws; `Input` only where the surface has a text field. */
export type Table = Pick<Elements['mobile'], 'Box' | 'Text' | 'Button' | 'Link' | 'Markdown'> & { Input?: Elements['desktop']['Input'] }

/** What a press or a submit in the pane does. */
export type Acts = {
  toggleFold: (stage: string) => void
  choose: (block: PaneBlock, label: string) => void
}

const text = (value: unknown) => (typeof value === 'string' ? value : '')
const bodyOf = (block: PaneBlock) => block.version.body
const FOLDED = new Set(['done', 'dropped'])

function header(t: Table, view: PaneView) {
  const { Box, Text, Link } = t
  return (
    <Box flexDirection="column">
      <Text bold>{`Yard Log of ${view.branch} · revision ${view.revision}`}</Text>
      {view.page ? <Link href={linkOf(view.page)} label={linkOf(view.page)} /> : null}
    </Box>
  )
}

function options(t: Table, block: PaneBlock, acts: Acts) {
  const { Box, Text, Button } = t
  const listed = Array.isArray(bodyOf(block).options) ? (bodyOf(block).options as Record<string, unknown>[]) : []
  return listed.map((option, n) => {
    const label = text(option.label)
    const shown = option.recommended === true ? `${label} (recommended)` : label
    return (
      <Box flexDirection="column">
        <Button key={`option-${block.key}-${n}`} label={shown} onPress={() => acts.choose(block, label)} />
        {text(option.detail) ? <Text dimColor>{text(option.detail)}</Text> : null}
      </Box>
    )
  })
}

/** A block that waits on the person, with what it asks. */
function asking(t: Table, block: PaneBlock, acts: Acts) {
  const { Box, Text, Markdown } = t
  const body = bodyOf(block)
  if (block.kind === 'question') {
    return (
      <Box flexDirection="column" marginTop={1}>
        <Text bold>{text(body.header)}</Text>
        <Text>{text(body.prompt)}</Text>
        {text(body.context) ? <Markdown text={text(body.context)} /> : null}
        {options(t, block, acts)}
      </Box>
    )
  }
  return (
    <Box flexDirection="column" marginTop={1}>
      <Text bold>{text(body.title)}</Text>
      <Markdown text={text(body.markdown)} />
    </Box>
  )
}

function waiting(t: Table, view: PaneView, acts: Acts) {
  const { Box, Text } = t
  const blocks = waitingOf(view)
  if (blocks.length === 0) return null
  return (
    <Box key="waiting" flexDirection="column" marginTop={1}>
      <Text bold color="yellow">Waits on you</Text>
      {blocks.map(each => asking(t, each, acts))}
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
  const status = text(bodyOf(stage).status)
  const foldable = FOLDED.has(status)
  const isFolded = foldable && !unfolded.includes(stage.key)
  return (
    <Box flexDirection="column" marginTop={1}>
      <Box flexDirection="row" gap={1}>
        <Text bold>{`${text(bodyOf(stage).title)} · ${status}`}</Text>
        {foldable ? <Button key={`unfold-${stage.key}`} label={isFolded ? 'Show' : 'Hide'} dimColor onPress={() => acts.toggleFold(stage.key)} /> : null}
      </Box>
      {isFolded ? null : (
        <Box flexDirection="column" paddingLeft={2}>
          {text(bodyOf(stage).goal) ? <Text dimColor>{text(bodyOf(stage).goal)}</Text> : null}
          {stage.blocks.map(each => held(t, each))}
        </Box>
      )}
    </Box>
  )
}

/** The pane's tree for what it holds. */
export function drawn(t: Table, view: PaneView | null, failure: string | null, kept: YardPaneDrafts, acts: Acts) {
  const { Box, Text } = t
  const failed = failure ? <Text color="red">{failure}</Text> : null
  if (view === null) return <Box flexDirection="column">{failed ?? <Text dimColor>Reading Yard Log…</Text>}</Box>
  if (!view.hasLog) {
    return (
      <Box flexDirection="column">
        <Text>{`No Yard Log on ${view.branch} yet.`}</Text>
        {failed}
      </Box>
    )
  }
  return (
    <Box flexDirection="column">
      {header(t, view)}
      {failed}
      {waiting(t, view, acts)}
      {view.stages.map(each => stageOf(t, each, kept.unfolded, acts))}
    </Box>
  )
}

/** What a press in the pane does, for this session. */
export function actsOf($: EngineInterface): Acts {
  return {
    toggleFold: stage =>
      void setDrafts($, before => ({
        ...before,
        unfolded: before.unfolded.includes(stage) ? before.unfolded.filter(each => each !== stage) : [...before.unfolded, stage],
      })),
    choose: () => {},
  }
}

/** Draws the pane on whichever surface asks. */
export function registerDraw(on: On) {
  on('ui.render', { component: 'Pane', requestId: PANE }, async ($, e) => {
    const t = $.ui.resolve(e) as unknown as Table
    const { view, failure } = await paneOf($)
    return drawn(t, view, failure, await draftsOf($), actsOf($))
  })
}
