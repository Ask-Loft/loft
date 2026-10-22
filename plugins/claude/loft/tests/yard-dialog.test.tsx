import { expect, test } from 'claude-code/testing'

import { PAGE, start, world, type On } from './yard-world'

const ASKING = {
  surface: 'terminal' as const,
  component: 'AskUserQuestion' as const,
  requestId: 'ask-1',
  props: {
    tool: 'AskUserQuestion',
    questions: [{ question: 'Which?', header: 'Pick', multiSelect: false, options: [{ label: 'A', description: '' }, { label: 'B', description: '' }] }],
  },
}

/** Every text a tree draws, in order. */
function textsOf(node: unknown): string[] {
  if (typeof node === 'string') return [node]
  if (Array.isArray(node)) return node.flatMap(textsOf)
  if (typeof node !== 'object' || node === null) return []
  const children = (node as { props?: { children?: unknown }; children?: unknown }).props?.children ?? (node as { children?: unknown }).children
  const texts = textsOf(children)
  return (node as { type?: unknown }).type === 'Text' ? [texts.join('')] : texts
}

const drawing = (on: On) =>
  // Claude Code's own drawing of the dialog.
  on('ui.render', { component: 'AskUserQuestion' }, () => ({ type: 'engine', ref: 0 }) as const)

test("Claude Code's dialog shows the page's address above what it asks, once a command has given it", async ($, on) => {
  world(on)
  drawing(on)
  await start($)
  await $.tool.call({ tool: 'mcp__loft__yard_read' })
  const drawn = await $.ui.render(ASKING)
  const texts = textsOf(drawn)
  expect(texts).toEqual([`Also on the Yard page: ${PAGE}`])
  expect(JSON.stringify(drawn)).toContain('"type":"engine"')
})

test('with Yard Client off, the dialog is drawn as Claude Code draws it', async ($, on) => {
  world(on, { protocol: 'none' })
  drawing(on)
  await start($)
  expect(await $.ui.render(ASKING)).toEqual({ type: 'engine', ref: 0 })
})
