// What waits on the person, as the Yard page's panel shows it: every open submission, the oldest
// first, each block it holds that still waits drawn ready to answer or decide. A question's options
// are lettered cards that choose, its field beneath them takes the person's own words; an approval
// or a plan has Approve, Reject and a field for the reason (the Feature "Keep the Yard page beside
// the session").

import type { PaneBlock, YardPaneDrafts } from '../../../types'
import { records, textOf as text } from '../../model'
import { bodyOf, type Drawing } from './kit'
import { letterOf } from './model'

const KINDS: Record<string, string> = { question: 'Question', approval: 'Approval', plan: 'Plan' }

/** A waiting card's head: its kind, a question's header as a chip, and that it waits, at the right. */
function cardHead({ t, palette }: Drawing, block: PaneBlock) {
  const { Box, Text } = t
  const header = block.kind === 'question' ? text(bodyOf(block).header) : ''
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
      </Box>
      <Text color={palette.wait} size="caption" weight="semibold">
        Waits on you
      </Text>
    </Box>
  )
}

function markdownOf({ t }: Drawing, words: unknown) {
  const { Markdown } = t
  return text(words) ? <Markdown text={text(words)} /> : null
}

/** A question's options as lettered cards, each choosing its option; a ticked one outlined. */
function options(drawing: Drawing, block: PaneBlock, kept: YardPaneDrafts) {
  const { t, palette, acts } = drawing
  const { Box, Text, Button } = t
  const body = bodyOf(block)
  const ticked = kept.ticked[block.key] ?? []
  const isMultiple = body.multiple === true
  return records(body.options).map((option, n) => {
    const label = text(option.label)
    const isTicked = ticked.includes(label)
    return (
      <Box key={`pick-${block.key}-${n}`} flexDirection="column" borderStyle="single" borderColor={isTicked ? palette.signal : palette.line} paddingX={1}>
        <Box flexDirection="row" gap={1}>
          <Text color={palette.signal} weight="semibold">
            {letterOf(n)}
          </Text>
          <Button key={`option-${block.key}-${n}`} plain label={isMultiple ? `${isTicked ? '☑' : '☐'} ${label}` : label} onPress={() => acts.choose(block, label)} />
          {option.recommended === true ? (
            <Text color={palette.recommended} size="caption" weight="semibold">
              Recommended
            </Text>
          ) : null}
        </Box>
        {text(option.detail) ? (
          <Text color={palette.muted} size="caption">
            {text(option.detail)}
          </Text>
        ) : null}
      </Box>
    )
  })
}

/** The field for the person's own words, or a reason, on a surface that has one. */
function field(drawing: Drawing, block: PaneBlock, kept: YardPaneDrafts, labels: { placeholder: string; submitLabel: string }, onSubmit: (words: string) => void) {
  const { Input } = drawing.t
  if (!Input) return null
  const prefix = block.kind === 'question' ? 'words' : 'reason'
  return (
    <Input
      key={`${prefix}-${block.key}`}
      placeholder={labels.placeholder}
      submitLabel={labels.submitLabel}
      value={kept.words[block.key] ?? ''}
      onInput={words => drawing.acts.type(block, words)}
      onSubmit={onSubmit}
    />
  )
}

/** What a waiting block asks, and the controls that answer it. */
function controls(drawing: Drawing, block: PaneBlock, kept: YardPaneDrafts) {
  const { t, palette, acts } = drawing
  const { Box, Text, Button } = t
  const body = bodyOf(block)
  if (block.kind === 'question') {
    return [
      <Text color={palette.ink}>{text(body.prompt)}</Text>,
      markdownOf(drawing, body.context),
      <Box flexDirection="column">{options(drawing, block, kept)}</Box>,
      body.ownWords === false ? null : field(drawing, block, kept, { placeholder: 'In your own words', submitLabel: 'Send' }, words => acts.send(block, words)),
      body.multiple === true ? <Button key={`send-${block.key}`} label="Send" variant="primary" onPress={() => acts.send(block)} /> : null,
    ]
  }
  return [
    text(body.title) ? (
      <Text color={palette.ink} weight="semibold">
        {text(body.title)}
      </Text>
    ) : null,
    markdownOf(drawing, body.markdown),
    field(drawing, block, kept, { placeholder: 'A reason, to reject', submitLabel: 'Reject' }, words => acts.decide(block, false, words)),
    <Box flexDirection="row" gap={1}>
      <Button key={`approve-${block.key}`} label="Approve" variant="primary" onPress={() => acts.decide(block, true)} />
      <Button key={`reject-${block.key}`} label="Reject" onPress={() => acts.decide(block, false)} />
    </Box>,
  ]
}

/** A waiting block's card, outlined in the waiting colour, with a bar of it at its left. */
function askCard(drawing: Drawing, block: PaneBlock, kept: YardPaneDrafts) {
  const { t, palette } = drawing
  const { Box } = t
  return (
    <Box key={`ask-${block.key}`} flexDirection="row" borderStyle="single" borderColor={palette.wait}>
      <Box key={`wait-bar-${block.key}`} width={1} backgroundColor={palette.wait} />
      <Box flexDirection="column" flexGrow={1} flexShrink={1} paddingX={1} gap={1}>
        {cardHead(drawing, block)}
        {controls(drawing, block, kept)}
      </Box>
    </Box>
  )
}

/** What waits on the person: a title that counts it, then each block, the oldest submission first. */
export function whatWaits(drawing: Drawing, blocks: readonly PaneBlock[], kept: YardPaneDrafts) {
  const { Box, Text } = drawing.t
  const n = blocks.length
  return (
    <Box key="waiting" flexDirection="column" gap={1} marginTop={1}>
      <Box key="waiting-title">
        <Text color={drawing.palette.ink} size="heading" weight="semibold">{`${n} ${n === 1 ? 'block waits' : 'blocks wait'} on you`}</Text>
      </Box>
      {blocks.map(each => askCard(drawing, each, kept))}
    </Box>
  )
}
