// The head of a block's card, as the Yard page draws it, with its title and Markdown: what the
// pane's cards of what waits are drawn with (the Feature "Keep the Yard page beside the session").

import type { PaneBlock } from '../../../types'
import { textOf as text } from '../../model'
import { bodyOf, type Drawing } from './kit'
import { blockStateOf } from './model'
import { statusColour } from './theme'

const KINDS: Record<string, string> = { note: 'Note', question: 'Question', approval: 'Approval', plan: 'Plan', decision: 'Decision', task: 'Task', loft: 'Loft' }

/** A card's head: its kind, a question's header as a chip or a fix's mark, and its state at the right. */
export function cardHead({ t, palette }: Drawing, block: PaneBlock, isWaiting: boolean) {
  const { Box, Text } = t
  const body = bodyOf(block)
  const state = blockStateOf(block, isWaiting)
  const header = block.kind === 'question' ? text(body.header) : ''
  return (
    <Box key={`card-head-${block.key}`} flexDirection="row" justifyContent="space-between" gap={1}>
      <Box flexDirection="row" gap={1}>
        <Text color={palette.muted} size="caption">
          {KINDS[block.kind] ?? block.kind}
        </Text>
        {header ? (
          <Box key={`chip-${block.key}`} borderStyle="single" borderColor={palette.line} paddingX={1}>
            <Text color={palette.muted} size="caption">
              {header}
            </Text>
          </Box>
        ) : null}
        {block.kind === 'task' && body.type === 'fix' ? (
          <Box key={`fix-${block.key}`}>
            <Text color={palette.fix} size="caption" weight="semibold">
              Fix
            </Text>
          </Box>
        ) : null}
      </Box>
      {state ? (
        <Text color={statusColour(palette, state[1])} size="caption" weight="semibold">
          {state[0]}
        </Text>
      ) : null}
    </Box>
  )
}

export function title({ t, palette }: Drawing, words: string) {
  const { Text } = t
  return words ? (
    <Text color={palette.ink} weight="semibold">
      {words}
    </Text>
  ) : null
}

export function markdownOf({ t }: Drawing, words: unknown) {
  const { Markdown } = t
  return text(words) ? <Markdown text={text(words)} /> : null
}
