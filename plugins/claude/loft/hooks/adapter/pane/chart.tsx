// The Yard pane's Gantt chart drawn: one SVG image where the surface draws SVG, with the stages
// beneath it as buttons that open each in the timeline, since an image takes no press; on the
// terminal, each row a bar of block characters, its stage's name the button (the Feature "See Yard
// Log in the pane as on the Yard page").

import type { PaneView } from '../../../types'
import { altOf, barsOf, ganttOf, svgOf, toneColour, type Gantt } from './gantt'
import type { Drawing } from './kit'
import { clockOf } from './model'

const LABEL = 22

function image(drawing: Drawing, chart: Gantt) {
  const { t, palette, acts } = drawing
  const { Box, Text, Button, Svg } = t
  if (!Svg) return null
  const stages = chart.rows.filter(row => row.kind === 'stage')
  return (
    <Box flexDirection="column" gap={1}>
      <Svg source={svgOf(chart, palette)} alt={altOf(chart)} />
      <Box flexDirection="row" flexWrap="wrap" gap={1}>
        <Text color={palette.muted} size="caption">
          Open in the timeline:
        </Text>
        {stages.map(row => (
          <Button key={`gantt-stage-${row.key}`} plain label={row.title} onPress={() => acts.pick(row.key)} />
        ))}
      </Box>
    </Box>
  )
}

function bars(drawing: Drawing, chart: Gantt, columns: number) {
  const { t, palette, acts } = drawing
  const { Box, Text, Button } = t
  const width = Math.max(10, columns - LABEL - 2)
  const titles = new Map(chart.rows.map(row => [row.key, row]))
  return (
    <Box flexDirection="column">
      <Box flexDirection="row">
        <Box width={LABEL} flexShrink={0} />
        <Box width={width} flexDirection="row" justifyContent="space-between">
          <Text color={palette.muted} size="caption">
            {clockOf(new Date(chart.start).toISOString())}
          </Text>
          <Text color={palette.muted} size="caption">
            {clockOf(new Date(chart.end).toISOString())}
          </Text>
        </Box>
      </Box>
      {barsOf(chart, width).map(({ key, runs }) => {
        const row = titles.get(key)!
        const name = row.title.length > LABEL - 3 ? `${row.title.slice(0, LABEL - 4)}…` : row.title
        return (
          <Box flexDirection="row">
            <Box width={LABEL} flexShrink={0} paddingLeft={row.kind === 'stage' ? 0 : 2}>
              {row.kind === 'stage' ? (
                <Button key={`gantt-stage-${key}`} plain label={name} onPress={() => acts.pick(key)} />
              ) : (
                <Text color={palette.muted}>{name}</Text>
              )}
            </Box>
            <Box key={`gantt-bar-${key}`} flexDirection="row" flexShrink={0}>
              {runs.map(run => (
                <Text color={run.tone === null ? undefined : toneColour(palette, run.tone)}>{run.text}</Text>
              ))}
            </Box>
          </Box>
        )
      })}
    </Box>
  )
}

/** The Gantt chart of `view` at `now`, `columns` wide. */
export function chart(drawing: Drawing, view: PaneView, now: number, columns: number) {
  const { Box, Text } = drawing.t
  const drawn = ganttOf(view, now)
  if (drawn.rows.length === 0) return <Text color={drawing.palette.muted}>No stage has started yet.</Text>
  return <Box key="gantt">{drawing.t.Svg ? image(drawing, drawn) : bars(drawing, drawn, columns)}</Box>
}
