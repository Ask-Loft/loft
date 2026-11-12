// The Yard pane drawn as the Yard page draws Yard Log (the Feature "See Yard Log in the pane as on
// the Yard page"), on every surface Claude Code draws a pane on.

import type { FoundElement } from 'claude-code/testing'
import { expect, test } from 'claude-code/testing'

import { DARK, isDark, LIGHT, paletteOf } from '../hooks/adapter/pane/theme'
import { block, openPane, PAGE, paneMount, readDoc, SESSION, stage, world } from './yard-world'

const SURFACES = ['terminal', 'desktop', 'vscode', 'mobile'] as const

/** The colour of the first Text an element holds. */
function colourIn(found: FoundElement | undefined): unknown {
  const first = found?.children.find(child => typeof child === 'object' && child !== null) as FoundElement | undefined
  if (!first) return undefined
  return first.type === 'Text' ? first.props.color : colourIn(first)
}
const OWNER = { host: 'claude-code', session: SESSION }

function staged(id: string, status: string, ...blocks: ReturnType<typeof block>[]) {
  return { ...stage(id, status, OWNER), blocks }
}

const LOG = readDoc({
  stages: [staged('shape', 'done'), staged('build', 'active', block('store', 'task', { title: 'Write the store', state: 'active' })), staged('review', 'planned')],
})

const ASKING = readDoc({
  stages: [
    staged(
      'shape',
      'active',
      block('readers', 'question', { prompt: 'Which readers?', header: 'Readers', options: [{ label: 'Members', recommended: true }, { label: 'Visitors' }] }),
      block('go', 'approval', { title: 'Approve the shape', markdown: 'All of it.' }),
    ),
  ],
  submissions: [{ agent: OWNER, open: [{ key: 's-1', state: 'open', blocks: ['k-readers', 'k-go'] }] }],
})

test("Claude Code's dark themes take the page's dark tokens, and every other theme its light ones", () => {
  expect(isDark('dark')).toBe(true)
  expect(isDark('dark-daltonized')).toBe(true)
  expect(isDark('light')).toBe(false)
  expect(isDark('auto')).toBe(false)
  expect(paletteOf(true)).toBe(DARK)
  expect(paletteOf(false)).toBe(LIGHT)
  expect(LIGHT.done).toBe('#0a756c')
  expect(LIGHT.signal).toBe('#14488f')
  expect(LIGHT.wait).toBe('#a8480c')
  expect(LIGHT.fix).toBe('#d23f74')
})

test("the head names the repository above the Yard Log's title, set larger, sums up its stages and links the page", async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount(paneMount(surface))
    expect(await ui.find({ key: 'head-kicker' })).toBeDefined()
    expect((await ui.find({ type: 'Text', text: 'shelf' }))?.props.color).toBe(LIGHT.muted)
    expect((await ui.find({ type: 'Text', text: 'feat/shelf' }))?.props.size).toBe('title')
    expect(await ui.find({ type: 'Text', text: '3 stages: 1 done, 1 active, 1 planned' })).toBeDefined()
    expect((await ui.find({ type: 'Link' }))?.props.href).toBe(PAGE.replace('127.0.0.1', 'localhost'))
    await ui.unmount()
  }
})

test('the head counts what waits on the person in a button of the waiting colour, and shows none where nothing waits', async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount(paneMount(surface))
    expect((await ui.find({ key: 'wait-count' }))?.text).toBe('2 wait on you')
    expect((await ui.find({ key: 'wait-control' }))?.props.backgroundColor).toBe(LIGHT.wait)
    expect(await ui.find({ type: 'Text', text: 'Asked of you' })).toBeDefined()
    await ui.unmount()
  }
  w.state.read = LOG
  await $.command.run({ command: 'yard-pane', args: '' } as never)
  const ui = await $.ui.mount(paneMount('desktop'))
  expect(await ui.find({ key: 'wait-count' })).toBeUndefined()
  await ui.unmount()
})

test('the tabs Timeline and Gantt chart choose the view, and the tab of the view shown is marked', async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount(paneMount(surface))
    expect((await ui.find({ key: 'tab-timeline' }))?.props.variant).toBe('primary')
    expect((await ui.find({ key: 'tab-gantt' }))?.props.variant).toBe('secondary')
    await ui.press({ key: 'tab-gantt' })
    expect((await ui.find({ key: 'tab-gantt' }))?.props.variant).toBe('primary')
    expect((await ui.find({ key: 'tab-timeline' }))?.props.variant).toBe('secondary')
    expect(await ui.find({ key: 'view-gantt' })).toBeDefined()
    await ui.press({ key: 'tab-timeline' })
    expect(await ui.find({ key: 'view-timeline' })).toBeDefined()
    await ui.unmount()
  }
})

test("under a dark theme the pane wears the page's dark tokens", async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  w.state.theme = 'dark'
  await openPane($)
  const ui = await $.ui.mount(paneMount('desktop'))
  expect((await ui.find({ key: 'wait-control' }))?.props.backgroundColor).toBe(DARK.wait)
  expect((await ui.find({ type: 'Text', text: 'shelf' }))?.props.color).toBe(DARK.muted)
  await ui.unmount()
})

const RAIL = readDoc({
  stages: [
    { ...staged('shape', 'done', block('choice', 'decision', { title: 'Members only', consequence: 'Visitors wait' })), times: { started: '2026-10-22T08:00:00Z', ended: '2026-10-22T08:30:00Z' } },
    {
      ...staged(
        'build',
        'active',
        block('store', 'task', { title: 'Write the store', state: 'active' }),
        block('go', 'approval', { title: 'Approve the store', markdown: 'All of it.' }),
      ),
      version: { body: { title: 'Build the store', goal: 'The store holds the shelf', status: 'active' } },
      times: { started: '2026-10-22T08:40:00Z' },
    },
    staged('review', 'planned', block('later', 'note', { markdown: 'Planned work.' })),
  ],
  submissions: [{ agent: OWNER, open: [{ key: 's-1', state: 'open', blocks: ['k-go'] }] }],
})

test("each stage's row shows a square and its status in the status's colour, its title, its owner and the count of what waits", async ($, on) => {
  const w = world(on)
  w.state.read = RAIL
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount(paneMount(surface))
    expect((await ui.findAll({ type: 'Text', text: '■' })).map(each => each.props.color)).toEqual([LIGHT.done, LIGHT.signal, LIGHT.rail])
    expect((await ui.find({ key: 'choose-k-build' }))?.text).toBe('Build the store')
    expect((await ui.find({ type: 'Text', text: 'Done' }))?.props.color).toBe(LIGHT.done)
    expect((await ui.find({ type: 'Text', text: 'Active' }))?.props.color).toBe(LIGHT.signal)
    expect((await ui.find({ type: 'Text', text: /^Planned$/ }))?.props.color).toBe(LIGHT.rail)
    expect(await ui.findAll({ type: 'Text', text: 'main agent' })).not.toHaveLength(0)
    expect((await ui.find({ type: 'Text', text: '1 waiting' }))?.props.color).toBe(LIGHT.wait)
    await ui.unmount()
  }
})

test('a line marks where the planned stages begin', async ($, on) => {
  const w = world(on)
  w.state.read = RAIL
  await openPane($)
  const ui = await $.ui.mount(paneMount('desktop'))
  const texts = (await ui.findAll({})).map(each => each.text)
  const line = texts.indexOf('Planned from here on')
  expect(line).toBeGreaterThan(texts.indexOf('Build the store'))
  expect(line).toBeLessThan(texts.indexOf('Stage review'))
  await ui.unmount()
})

test("the chosen stage's row is open to its blocks, the others folded, and a row opens and folds as the person chooses", async ($, on) => {
  const w = world(on)
  w.state.read = RAIL
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount(paneMount(surface))
    expect(await ui.find({ key: 'block-row-k-store' })).toBeDefined()
    expect((await ui.find({ key: 'block-state-k-store' }))?.text).toBe('Active')
    expect((await ui.find({ key: 'block-state-k-go' }))?.text).toBe('Waits on you')
    expect(colourIn(await ui.find({ key: 'block-state-k-go' }))).toBe(LIGHT.wait)
    expect((await ui.find({ key: 'row-k-build' }))?.props.backgroundColor).toBe(LIGHT.mist)
    expect((await ui.find({ key: 'row-k-shape' }))?.props.backgroundColor).toBeUndefined()
    expect(await ui.find({ key: 'block-row-k-choice' })).toBeUndefined()
    await ui.press({ key: 'fold-k-shape' })
    expect((await ui.find({ key: 'block-row-k-choice' }))?.text).toContain('Members only')
    await ui.press({ key: 'fold-k-shape' })
    expect(await ui.find({ key: 'block-row-k-choice' })).toBeUndefined()
    await ui.press({ key: 'fold-k-build' })
    expect(await ui.find({ key: 'block-row-k-store' })).toBeUndefined()
    await ui.press({ key: 'fold-k-build' })
    await ui.unmount()
  }
})

test('the stage chosen is the first active one until the person chooses another, and shows in full', async ($, on) => {
  const w = world(on)
  w.state.read = RAIL
  await openPane($)
  const ui = await $.ui.mount(paneMount('desktop'))
  expect((await ui.find({ key: 'stage-title' }))?.text).toBe('Build the store')
  expect(await ui.find({ type: 'Text', text: 'The store holds the shelf' })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: '2 blocks' })).toBeDefined()
  expect((await ui.find({ type: 'Text', text: /^Started / }))?.text).not.toContain('ended')
  await ui.press({ key: 'choose-k-shape' })
  expect((await ui.find({ key: 'stage-title' }))?.text).toBe('Stage shape')
  expect((await ui.find({ type: 'Text', text: /^Started / }))?.text).toContain(', ended ')
  expect(await ui.find({ key: 'block-row-k-choice' })).toBeDefined()
  await ui.unmount()
})

test('with no active stage, the stage chosen is the first', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [staged('shape', 'done'), staged('review', 'planned')] })
  await openPane($)
  const ui = await $.ui.mount(paneMount('terminal'))
  expect((await ui.find({ key: 'stage-title' }))?.text).toBe('Stage shape')
  await ui.unmount()
})

test('from 100 columns the rail and the stage chosen stand side by side, and below that one above the other', async ($, on) => {
  const w = world(on)
  w.state.read = RAIL
  await openPane($)
  for (const surface of SURFACES) {
    const wide = await $.ui.mount(paneMount(surface, 120))
    expect((await wide.find({ key: 'timeline' }))?.props.flexDirection).toBe('row')
    await wide.unmount()
    const narrow = await $.ui.mount(paneMount(surface, 60))
    expect((await narrow.find({ key: 'timeline' }))?.props.flexDirection).toBe('column')
    await narrow.unmount()
  }
})
