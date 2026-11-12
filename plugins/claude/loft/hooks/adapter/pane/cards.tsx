// The cards of the Yard pane's blocks, by their kind, and the controls that answer what waits
// (the Feature "See Yard Log in the pane as on the Yard page").

import type { PaneBlock, YardPaneDrafts } from '../../../types'
import { textOf as text } from '../../model'
import { bodyOf, type Acts, type Table } from './kit'
import { answeredOn } from './model'

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

/** One block of a stage, as the stage holds it. */
export function held(t: Table, block: PaneBlock) {
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

