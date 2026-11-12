// The stage in hand: the first `active` stage, the agent that owns it and its goal, with its
// `active` tasks beneath, a fix marked `Fix`; or that no stage is active. The Yard page beside the
// pane shows the rest (the Feature "Keep the Yard page beside the session").

import type { PaneView } from '../../../types'
import { textOf as text } from '../../model'
import { bodyOf, type Drawing } from './kit'
import { ownerOf } from './model'

export function stageInHand({ t, palette }: Drawing, view: PaneView) {
  const { Box, Text } = t
  const stage = view.stages.find(each => bodyOf(each).status === 'active')
  if (!stage) {
    return (
      <Box key="stage-in-hand" marginTop={1}>
        <Text color={palette.muted}>No stage is active.</Text>
      </Box>
    )
  }
  const body = bodyOf(stage)
  const owner = ownerOf(stage.owner)
  const tasks = stage.blocks.filter(block => block.kind === 'task' && bodyOf(block).state === 'active')
  return (
    <Box key="stage-in-hand" flexDirection="column" marginTop={1}>
      <Text>
        <Text color={palette.signal} weight="semibold">
          Active
        </Text>
        <Text color={palette.muted}>{owner ? `  ${owner}` : ''}</Text>
      </Text>
      <Text color={palette.ink} size="heading" weight="semibold">
        {text(body.title)}
      </Text>
      {text(body.goal) ? <Text color={palette.body}>{text(body.goal)}</Text> : null}
      {tasks.map(task => (
        <Box key={`task-${task.key}`} flexDirection="row" gap={1}>
          <Text color={palette.signal}>●</Text>
          <Text color={palette.ink}>{text(bodyOf(task).title)}</Text>
          {bodyOf(task).type === 'fix' ? (
            <Box key={`fix-${task.key}`}>
              <Text color={palette.fix} size="caption" weight="semibold">
                Fix
              </Text>
            </Box>
          ) : null}
        </Box>
      ))}
    </Box>
  )
}
