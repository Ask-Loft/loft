// The Yard pane drawn as the Yard page draws Yard Log (the Feature "See Yard Log in the pane as on
// the Yard page"), on every surface Claude Code draws a pane on.

import { expect, test } from 'claude-code/testing'

import { DARK, isDark, LIGHT, paletteOf } from '../hooks/adapter/pane/theme'
import { block, openPane, PAGE, paneMount, readDoc, SESSION, stage, world } from './yard-world'

const SURFACES = ['terminal', 'desktop', 'vscode', 'mobile'] as const
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
