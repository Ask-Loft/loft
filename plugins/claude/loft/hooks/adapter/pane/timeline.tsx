// The Yard pane's timeline, as the Yard page draws it: the stages on a rail, each row with a square
// and its status in the status's colour, its title, its owner and the count of what waits, a line
// where the planned stages begin, the chosen stage's row open to its blocks; and beside the rail,
// or under it where the pane is narrow, the stage chosen in full (the Feature "See Yard Log in the
// pane as on the Yard page").

import type { PaneBlock, PaneStage, PaneView, YardPaneDrafts } from '../../../types'
import { textOf as text } from '../../model'
import { card } from './cards'
import { bodyOf, type Drawing } from './kit'
import { blockStateOf, blockTitle, chosenOf, clockOf, ownerOf, plannedFrom } from './model'
import { statusColour } from './theme'

/** The pane's width, in columns, from which the rail and the stage chosen stand side by side. */
export const SIDE_BY_SIDE = 100

const STATUS: Record<string, string> = { planned: 'Planned', active: 'Active', done: 'Done', dropped: 'Dropped' }

/** A block's kind as a glyph on its row, as the page gives it an icon. */
const GLYPH: Record<string, string> = { note: '▤', question: '?', approval: '✓', plan: '☰', decision: '◆', task: '□', loft: '#' }

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`

function blockRow({ t, palette, acts }: Drawing, stage: PaneStage, block: PaneBlock, waits: ReadonlySet<string>) {
  const { Box, Text, Button } = t
  const isWaiting = waits.has(block.key)
  const glyph = block.kind === 'task' && bodyOf(block).state === 'active' ? '●' : (GLYPH[block.kind] ?? '·')
  const state = blockStateOf(block, isWaiting)
  return (
    <Box flexDirection="row" gap={1}>
      <Text color={isWaiting ? palette.wait : palette.line}>▌</Text>
      <Text color={glyph === '●' ? palette.signal : palette.muted}>{glyph}</Text>
      <Box flexShrink={1}>
        <Button key={`block-row-${block.key}`} plain dimColor label={blockTitle(block)} onPress={() => acts.pick(stage.key)} />
      </Box>
      {state ? (
        <Box key={`block-state-${block.key}`}>
          <Text color={statusColour(palette, state[1])} size="caption">
            {state[0]}
          </Text>
        </Box>
      ) : null}
    </Box>
  )
}

function stageRow(drawing: Drawing, stage: PaneStage, isChosen: boolean, isOpen: boolean, waits: ReadonlySet<string>) {
  const { t, palette, acts } = drawing
  const { Box, Text, Button } = t
  const status = text(bodyOf(stage).status)
  const colour = statusColour(palette, status)
  const owner = ownerOf(stage.owner)
  const waiting = stage.blocks.filter(block => waits.has(block.key)).length
  return (
    <Box flexDirection="column">
      <Box key={`row-${stage.key}`} flexDirection="row" gap={1} backgroundColor={isChosen ? palette.mist : undefined}>
        <Text color={isChosen ? palette.signal : palette.canvas}>▌</Text>
        <Text color={colour}>■</Text>
        <Button key={`fold-${stage.key}`} plain dimColor label={isOpen ? '⌄' : '›'} onPress={() => acts.toggleFold(stage.key)} />
        <Box flexDirection="column" flexShrink={1}>
          <Button key={`choose-${stage.key}`} plain label={text(bodyOf(stage).title)} onPress={() => acts.pick(stage.key)} />
          <Box flexDirection="row" gap={1}>
            <Text color={colour} size="caption" weight="semibold">
              {STATUS[status] ?? status}
            </Text>
            {owner ? (
              <Text color={palette.muted} size="caption">
                {owner}
              </Text>
            ) : null}
            {waiting > 0 ? (
              <Text color={palette.wait} size="caption">{`${waiting} waiting`}</Text>
            ) : null}
          </Box>
        </Box>
      </Box>
      {isOpen ? (
        <Box flexDirection="column" paddingLeft={4}>
          {stage.blocks.map(block => blockRow(drawing, stage, block, waits))}
        </Box>
      ) : null}
    </Box>
  )
}

function rail(drawing: Drawing, view: PaneView, chosen: PaneStage | undefined, kept: YardPaneDrafts, waits: ReadonlySet<string>) {
  const { Box, Text } = drawing.t
  const line = plannedFrom(view.stages)
  return (
    <Box flexDirection="column">
      {view.stages.flatMap((stage, n) => {
        const isChosen = stage === chosen
        const isOpen = isChosen !== kept.toggled.includes(stage.key)
        const row = stageRow(drawing, stage, isChosen, isOpen, waits)
        if (n !== line || n === 0) return [row]
        return [
          <Text color={drawing.palette.muted} size="caption">
            Planned from here on
          </Text>,
          row,
        ]
      })}
    </Box>
  )
}

function detail(drawing: Drawing, stage: PaneStage, waits: ReadonlySet<string>) {
  const { t, palette } = drawing
  const { Box, Text } = t
  const body = bodyOf(stage)
  const status = text(body.status)
  const owner = ownerOf(stage.owner)
  const times = stage.times ?? {}
  return (
    <Box flexDirection="column" flexGrow={1} flexShrink={1}>
      <Box flexDirection="row" gap={1}>
        <Text color={statusColour(palette, status)} size="caption" weight="semibold">{`● ${STATUS[status] ?? status}`}</Text>
        {owner ? (
          <Text color={palette.muted} size="caption">
            {owner}
          </Text>
        ) : null}
        <Text color={palette.muted} size="caption">
          {plural(stage.blocks.length, 'block')}
        </Text>
      </Box>
      <Box key="stage-title">
        <Text color={palette.ink} size="heading" weight="semibold">
          {text(body.title)}
        </Text>
      </Box>
      {text(body.goal) ? <Text color={palette.body}>{text(body.goal)}</Text> : null}
      {times.started ? (
        <Text color={palette.muted} size="caption">{`Started ${clockOf(times.started)}${times.ended ? `, ended ${clockOf(times.ended)}` : ''}`}</Text>
      ) : null}
      <Box flexDirection="column" gap={1} marginTop={1}>
        {stage.blocks.map(block => card(drawing, block, waits.has(block.key)))}
      </Box>
    </Box>
  )
}

/** The timeline: the rail and the stage chosen, side by side from `SIDE_BY_SIDE` columns, else one above the other. */
export function timeline(drawing: Drawing, view: PaneView, kept: YardPaneDrafts, waits: ReadonlySet<string>, columns: number) {
  const { Box, Text } = drawing.t
  if (view.stages.length === 0) return <Text color={drawing.palette.muted}>No stage has been written yet.</Text>
  const chosen = chosenOf(view.stages, kept.chosen)
  const isWide = columns >= SIDE_BY_SIDE
  return (
    <Box key="timeline" flexDirection={isWide ? 'row' : 'column'} gap={isWide ? 3 : 1} marginTop={1}>
      <Box flexDirection="column" width={isWide ? Math.min(44, Math.floor(columns * 0.4)) : undefined} flexShrink={0}>
        {rail(drawing, view, chosen, kept, waits)}
      </Box>
      {chosen ? detail(drawing, chosen, waits) : null}
    </Box>
  )
}
