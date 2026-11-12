// The cards of the Yard pane's blocks, by their kind, and the controls that answer what waits
// (the Feature "See Yard Log in the pane as on the Yard page").

import type { PaneBlock, YardPaneDrafts } from '../../../types'
import { records, textOf as text } from '../../model'
import { bodyOf, type Acts, type Drawing, type Table } from './kit'
import { answeredOn, blockStateOf, channelWords, clockOf, letterOf, VIEW_NAMES, whoOf } from './model'
import { statusColour } from './theme'

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
export function asking(t: Table, block: PaneBlock, kept: YardPaneDrafts, acts: Acts) {
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

const KINDS: Record<string, string> = { note: 'Note', question: 'Question', approval: 'Approval', plan: 'Plan', decision: 'Decision', task: 'Task', loft: 'Loft' }

/** A card's head: its kind, a question's header as a chip or a fix's mark, and its state at the right. */
function cardHead({ t, palette }: Drawing, block: PaneBlock, isWaiting: boolean) {
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
          <Box key={`chip-${block.key}`} backgroundColor={palette.surface} paddingX={1}>
            <Text color={palette.body} size="caption">
              {header}
            </Text>
          </Box>
        ) : null}
        {block.kind === 'task' && body.type === 'fix' ? (
          <Box key={`fix-${block.key}`} backgroundColor={palette.fixPale} paddingX={1}>
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

function title({ t, palette }: Drawing, words: string) {
  const { Text } = t
  return words ? (
    <Text color={palette.ink} weight="semibold">
      {words}
    </Text>
  ) : null
}

function markdownOf({ t }: Drawing, words: unknown) {
  const { Markdown } = t
  return text(words) ? <Markdown text={text(words)} /> : null
}

/** An option's card, lettered, outlined in action blue once chosen, `Recommended` where it is. */
function optionCard({ t, palette }: Drawing, key: string, index: number, label: string, detail: string, isChosen: boolean, isRecommended: boolean) {
  const { Box, Text } = t
  return (
    <Box key={key} flexDirection="column" borderStyle="single" borderColor={isChosen ? palette.signal : palette.line} paddingX={1}>
      <Box flexDirection="row" gap={1}>
        <Text color={palette.signal} weight="semibold">
          {letterOf(index)}
        </Text>
        <Text color={palette.ink}>{label}</Text>
        {isRecommended ? (
          <Box backgroundColor={palette.recommendedSoft} paddingX={1}>
            <Text color={palette.recommended} size="caption">
              Recommended
            </Text>
          </Box>
        ) : null}
      </Box>
      {detail ? (
        <Text color={palette.muted} size="caption">
          {detail}
        </Text>
      ) : null}
    </Box>
  )
}

function question(drawing: Drawing, block: PaneBlock) {
  const { t, palette } = drawing
  const { Box, Text } = t
  const body = bodyOf(block)
  const answer = block.answer ?? null
  const chosen = new Set(Array.isArray(answer?.labels) ? answer.labels.map(String) : [])
  const options = records(body.options)
  const words = text(answer?.words)
  const answered = answer ? [...chosen, ...(words ? [`“${words}”`] : [])].join(', ') || 'no option' : ''
  return [
    <Text color={palette.ink}>{text(body.prompt)}</Text>,
    answer ? (
      <Box key={`answer-${block.key}`} flexDirection="row" gap={1}>
        <Text color={palette.ink} weight="semibold">
          {answered}
        </Text>
        <Text color={palette.muted}>{`answered ${channelWords(answeredOn(block))}`.trim()}</Text>
      </Box>
    ) : null,
    markdownOf(drawing, body.context),
    <Box flexDirection="column" gap={0}>
      {options.map((option, n) =>
        optionCard(drawing, `card-option-${block.key}-${n}`, n, text(option.label), text(option.detail), chosen.has(text(option.label)), option.recommended === true),
      )}
      {words ? optionCard(drawing, `card-option-${block.key}-${options.length}`, options.length, 'In your own words', words, true, false) : null}
    </Box>,
  ]
}

function decidable(drawing: Drawing, block: PaneBlock) {
  const { t, palette } = drawing
  const { Box, Text } = t
  const body = bodyOf(block)
  const outcome = block.outcome ?? null
  const isApproved = outcome?.verdict === 'approved'
  const where = channelWords(answeredOn(block))
  const reason = text(outcome?.reason)
  const why = isApproved ? '' : reason ? `: ${reason}` : ', with no reason given'
  return [
    title(drawing, text(body.title)),
    outcome ? (
      <Box key={`outcome-${block.key}`} flexDirection="row">
        <Text color={isApproved ? palette.done : palette.danger} weight="semibold">
          {isApproved ? 'Approved' : 'Rejected'}
        </Text>
        <Text color={palette.body}>{`${where ? ` ${where}` : ''}${why}`}</Text>
      </Box>
    ) : null,
    text(body.path) ? (
      <Text color={palette.muted} size="caption">
        {text(body.path)}
      </Text>
    ) : null,
    markdownOf(drawing, body.markdown),
  ]
}

function content(drawing: Drawing, block: PaneBlock) {
  const { t, palette } = drawing
  const { Text } = t
  const body = bodyOf(block)
  switch (block.kind) {
    case 'question':
      return question(drawing, block)
    case 'approval':
    case 'plan':
      return decidable(drawing, block)
    case 'task':
      return [title(drawing, text(body.title)), markdownOf(drawing, body.detail)]
    case 'note':
      return [title(drawing, text(body.title)), markdownOf(drawing, body.markdown)]
    case 'decision':
      return [title(drawing, text(body.title)), <Text color={palette.body}>{text(body.consequence)}</Text>]
    case 'loft':
      return [title(drawing, VIEW_NAMES[text(body.view)] ?? text(body.view)), markdownOf(drawing, body.note)]
    default:
      return [<Text color={palette.muted}>{`A ${block.kind} block, which the pane does not draw.`}</Text>]
  }
}

/** A card's foot: who wrote the block, its version, and when it was created and last updated. */
function foot({ t, palette }: Drawing, block: PaneBlock) {
  const { Box, Text } = t
  const kept = block.foot
  if (!kept) return null
  const parts = [
    whoOf(kept.by),
    `version ${kept.number}`,
    `Created ${kept.created ? clockOf(kept.created) : 'unavailable'}`,
    ...(kept.number > 1 && kept.updated ? [`Updated ${clockOf(kept.updated)}`] : []),
  ]
  return (
    <Box key={`foot-${block.key}`} flexDirection="row" flexWrap="wrap" gap={2} marginTop={1}>
      {parts.map(part => (
        <Text color={palette.muted} size="caption">
          {part}
        </Text>
      ))}
    </Box>
  )
}

/**
 * A block's card, as the page draws it: a head, what its kind shows, and a foot; a block that
 * waits on the person on the waiting ground, with a bar of the waiting colour at its left.
 */
export function card(drawing: Drawing, block: PaneBlock, isWaiting: boolean) {
  const { t, palette, acts } = drawing
  const { Box, Text, Button } = t
  const asks = block.kind === 'question' ? 'Answer it' : 'Decide it'
  return (
    <Box
      key={`card-${block.key}`}
      flexDirection="row"
      borderStyle="single"
      borderColor={isWaiting ? palette.waitLine : palette.line}
      backgroundColor={isWaiting ? palette.waitSoft : undefined}
    >
      {isWaiting ? <Box key={`wait-bar-${block.key}`} width={1} backgroundColor={palette.wait} /> : null}
      <Box flexDirection="column" flexGrow={1} flexShrink={1} paddingX={1}>
        {cardHead(drawing, block, isWaiting)}
        {content(drawing, block)}
        {isWaiting ? (
          <Box flexDirection="row" gap={1} marginTop={1}>
            <Text color={palette.wait}>This waits on you.</Text>
            <Button
              key={`${block.kind === 'question' ? 'answer' : 'decide'}-it-${block.key}`}
              plain
              label={asks}
              onPress={() => acts.show('waiting')}
            />
          </Box>
        ) : null}
        {foot(drawing, block)}
      </Box>
    </Box>
  )
}
