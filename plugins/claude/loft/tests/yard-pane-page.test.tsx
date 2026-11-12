// The Yard pane as the Yard page's companion (the Feature "Keep the Yard page beside the session"):
// the page opened in Claude Code Desktop's browser pane, and in the pane the head, the stage in hand
// and what waits on the person, on every surface Claude Code draws a pane on.

import type { FoundElement } from 'claude-code/testing'
import { expect, test } from 'claude-code/testing'

import { PALETTE, STATUS_SHADES } from '../hooks/adapter/pane/theme'
import { block, openPane, PAGE, paneMount, readDoc, SESSION, stage, world } from './yard-world'

const SURFACES = ['terminal', 'desktop', 'vscode', 'mobile'] as const
const OWNER = { host: 'claude-code', session: SESSION }

/** The colour of the first coloured Text an element holds. */
function colourIn(found: { props?: Record<string, unknown>; type?: string; children?: unknown[] } | undefined): unknown {
  if (!found) return undefined
  if (found.type === 'Text' && found.props?.color !== undefined) return found.props.color
  for (const child of found.children ?? []) {
    const colour = typeof child === 'object' && child !== null ? colourIn(child as FoundElement) : undefined
    if (colour !== undefined) return colour
  }
  return undefined
}

/** The contrast of two `#rrggbb` colours, as WCAG measures it. */
function contrast(one: string, other: string) {
  const luminance = (hex: string) => {
    const [r, g, b] = [1, 3, 5].map(at => parseInt(hex.slice(at, at + 2), 16) / 255).map(c => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4))
    return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!
  }
  const [high, low] = [luminance(one), luminance(other)].sort((a, b) => b - a)
  return (high! + 0.05) / (low! + 0.05)
}

function staged(id: string, status: string, ...blocks: ReturnType<typeof block>[]) {
  return { ...stage(id, status, OWNER), blocks }
}

/** A stage with its goal. */
function goaled(id: string, status: string, goal: string, ...blocks: ReturnType<typeof block>[]) {
  const held = staged(id, status, ...blocks)
  return { ...held, version: { body: { ...held.version.body, goal } } }
}

const LOG = readDoc({
  stages: [staged('shape', 'done'), staged('build', 'active', block('store', 'task', { title: 'Write the store', state: 'active' })), staged('review', 'planned')],
})

const READERS = { prompt: 'Which readers?', header: 'Readers', context: 'Cards are **new**.', options: [{ label: 'Members', detail: 'Readers who hold a card', recommended: true }, { label: 'Visitors', detail: 'Anyone at the desk' }] }

const ASKING = readDoc({
  stages: [staged('shape', 'active', block('readers', 'question', READERS), block('go', 'approval', { title: 'Approve the shape', markdown: 'All of it.' }))],
  submissions: [{ agent: OWNER, open: [{ key: 's-1', state: 'open', blocks: ['k-readers', 'k-go'] }] }],
})

const IN_HAND = readDoc({
  stages: [
    goaled('shape', 'done', 'The change is shaped', block('old', 'task', { title: 'Shape it', state: 'active' })),
    goaled(
      'build',
      'active',
      'The store holds the shelf',
      block('store', 'task', { title: 'Write the store', state: 'active' }),
      block('mend', 'task', { title: 'Mend the reload', state: 'active', type: 'fix' }),
      block('later', 'task', { title: 'Tidy up', state: 'open' }),
      block('first', 'task', { title: 'Read the shelf', state: 'done' }),
    ),
    goaled('ship', 'active', 'Shipped', block('pack', 'task', { title: 'Pack it', state: 'active' })),
  ],
})

const recorded = (w: ReturnType<typeof world>) => w.state.runs.filter(run => run.argv.includes('answer')).map(run => JSON.parse(run.stdin ?? '[]'))

test("text, muted text and lines take Claude Code's own colours, and each status shade reads on a light ground and on a dark one", () => {
  expect([PALETTE.ink, PALETTE.body, PALETTE.muted, PALETTE.line]).toEqual(['text', 'text', 'inactive', 'subtle'])
  for (const shade of STATUS_SHADES) {
    expect(contrast(PALETTE[shade], '#ffffff')).toBeGreaterThanOrEqual(3)
    expect(contrast(PALETTE[shade], '#0c1117')).toBeGreaterThanOrEqual(3)
  }
})

test('on every surface the pane draws its text in Claude Code\'s own colours, and no tinted ground but the rust of what waits', async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount(paneMount(surface))
    expect((await ui.find({ type: 'Text', text: 'feat/shelf' }))?.props.color).toBe('text')
    expect((await ui.find({ type: 'Text', text: 'shelf' }))?.props.color).toBe('inactive')
    const grounds = (await ui.findAll({})).flatMap(each => (each.props.backgroundColor === undefined ? [] : [each.props.backgroundColor]))
    expect(new Set(grounds)).toEqual(new Set([PALETTE.wait]))
    await ui.unmount()
  }
})

test("the head names the repository above the Yard Log's title, set larger, sums up its stages and links the page", async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount(paneMount(surface))
    expect(await ui.find({ key: 'head-kicker' })).toBeDefined()
    expect((await ui.find({ type: 'Text', text: 'shelf' }))?.props.color).toBe(PALETTE.muted)
    expect((await ui.find({ type: 'Text', text: 'feat/shelf' }))?.props.size).toBe('title')
    expect(await ui.find({ type: 'Text', text: '3 stages: 1 done, 1 active, 1 planned' })).toBeDefined()
    expect((await ui.find({ type: 'Link' }))?.props.href).toBe(PAGE.replace('127.0.0.1', 'localhost'))
    await ui.unmount()
  }
})

test('the head counts what waits on the person in the waiting colour, and shows no count where nothing waits', async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount(paneMount(surface))
    expect((await ui.find({ key: 'wait-count' }))?.text).toBe('2 wait on you')
    expect((await ui.find({ key: 'wait-control' }))?.props.backgroundColor).toBe(PALETTE.wait)
    await ui.unmount()
  }
  w.state.read = LOG
  await $.command.run({ command: 'yard-pane', args: '' } as never)
  const ui = await $.ui.mount(paneMount('desktop'))
  expect(await ui.find({ key: 'wait-count' })).toBeUndefined()
  await ui.unmount()
})

test("the pane shows the first active stage's title, the agent that owns it and its goal", async ($, on) => {
  const w = world(on)
  w.state.read = IN_HAND
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount(paneMount(surface))
    const shown = await ui.find({ key: 'stage-in-hand' })
    expect(shown?.text).toContain('Stage build')
    expect(shown?.text).toContain('main agent')
    expect(shown?.text).toContain('The store holds the shelf')
    expect(shown?.text).not.toContain('Stage ship')
    expect(colourIn(shown)).toBe(PALETTE.signal)
    await ui.unmount()
  }
})

test('each active task of the stage in hand shows with its title, a fix marked Fix, and no other task', async ($, on) => {
  const w = world(on)
  w.state.read = IN_HAND
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount(paneMount(surface))
    const tasks = (await ui.findAll({})).flatMap(each => (each.key?.startsWith('task-') ? [each.key] : []))
    expect(tasks).toEqual(['task-k-store', 'task-k-mend'])
    expect((await ui.find({ key: 'task-k-mend' }))?.text).toMatch(/Mend the reload.*Fix/)
    expect(colourIn(await ui.find({ key: 'fix-k-mend' }))).toBe(PALETTE.fix)
    expect(await ui.find({ key: 'fix-k-store' })).toBeUndefined()
    await ui.unmount()
  }
})

test('where no stage is active, the pane says so', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [staged('shape', 'done'), staged('review', 'planned')] })
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount(paneMount(surface))
    expect((await ui.find({ key: 'stage-in-hand' }))?.text).toBe('No stage is active.')
    await ui.unmount()
  }
})

test('what waits shows without a press, the oldest submission first, each submission in its own order', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({
    stages: (ASKING.stages as object[]).concat([staged('later', 'planned', block('more', 'approval', { title: 'More', markdown: 'More.' }))]),
    submissions: [
      { agent: OWNER, open: [{ key: 's-new', state: 'open', blocks: ['k-readers', 'k-more'], sent: { at: '2026-10-22T10:00:00Z' } }] },
      { agent: { ...OWNER, agent: 'sub-1' }, open: [{ key: 's-old', state: 'open', blocks: ['k-go'], sent: { at: '2026-10-22T09:00:00Z' } }] },
    ],
  })
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount(paneMount(surface))
    const asks = (await ui.findAll({})).flatMap(each => (each.key?.startsWith('ask-') ? [each.key] : []))
    expect(asks).toEqual(['ask-k-go', 'ask-k-readers', 'ask-k-more'])
    expect((await ui.find({ key: 'waiting-title' }))?.text).toBe('3 blocks wait on you')
    await ui.unmount()
  }
})

test('the card of a block that waits is outlined in the waiting colour, with a bar of it at its left', async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount(paneMount(surface))
    for (const key of ['k-readers', 'k-go']) {
      expect((await ui.find({ key: `ask-${key}` }))?.props.borderColor).toBe(PALETTE.wait)
      expect((await ui.find({ key: `wait-bar-${key}` }))?.props.backgroundColor).toBe(PALETTE.wait)
    }
    await ui.unmount()
  }
})

test('a question shows its prompt, its context as Markdown and each option as a card lettered A, B and on, the recommended one marked', async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount(paneMount(surface))
    expect(await ui.find({ type: 'Text', text: 'Which readers?' })).toBeDefined()
    expect(await ui.find({ type: 'Markdown', text: 'Cards are **new**.' })).toBeDefined()
    expect((await ui.find({ key: 'pick-k-readers-0' }))?.text).toMatch(/^A.*Members.*Recommended.*Readers who hold a card$/)
    expect((await ui.find({ key: 'pick-k-readers-1' }))?.text).toMatch(/^B.*Visitors.*Anyone at the desk$/)
    expect((await ui.find({ key: 'pick-k-readers-1' }))?.text).not.toContain('Recommended')
    await ui.unmount()
  }
})

test("a question's lettered option cards choose its options, and a field beneath them takes the person's own words", async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  await openPane($)
  const ui = await $.ui.mount(paneMount('desktop'))
  expect((await ui.find({ key: 'pick-k-readers-0' }))?.text).toMatch(/^A.*Members.*Recommended/)
  expect((await ui.find({ key: 'pick-k-readers-1' }))?.text).toMatch(/^B.*Visitors/)
  const keys = (await ui.findAll({})).map(each => each.key)
  expect(keys.indexOf('words-k-readers')).toBeGreaterThan(keys.indexOf('pick-k-readers-1'))
  await ui.press({ key: 'option-k-readers-1' })
  expect(recorded(w)).toEqual([[{ block: 'k-readers', labels: ['Visitors'] }]])
  await ui.unmount()
})

test('an approval shows its title and its Markdown, with Approve and Reject', async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount(paneMount(surface))
    expect((await ui.find({ key: 'ask-k-go' }))?.text).toContain('Approve the shape')
    expect(await ui.find({ type: 'Markdown', text: 'All of it.' })).toBeDefined()
    expect((await ui.find({ key: 'approve-k-go' }))?.props.label).toBe('Approve')
    expect((await ui.find({ key: 'reject-k-go' }))?.props.label).toBe('Reject')
    await ui.unmount()
  }
})

test("in Claude Code Desktop the command opens the Yard page at the session's tree and branch in Desktop's browser pane", async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  w.state.surfaces = ['terminal', 'desktop']
  await openPane($)
  expect(w.state.browser.calls.map(call => call.tool)).toEqual(['tabs_context', 'preview_start'])
  expect(w.state.browser.calls[1]!.args.url).toBe(PAGE)
  expect([...w.state.browser.tabs.values()]).toEqual([PAGE])
})

test("where Desktop's browser pane already shows the Yard page, the command shows that tab and opens no other", async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  w.state.surfaces = ['desktop']
  await openPane($)
  await $.command.run({ command: 'yard-pane', args: '' } as never)
  expect(w.state.browser.calls.map(call => call.tool)).toEqual(['tabs_context', 'preview_start', 'tabs_context', 'navigate'])
  expect(w.state.browser.calls[3]!.args).toEqual({ tabId: 'tab-1', url: PAGE })
  expect(w.state.browser.tabs.size).toBe(1)
  w.state.browser.tabs.clear()
  await $.command.run({ command: 'yard-pane', args: '' } as never)
  expect(w.state.browser.calls.slice(4).map(call => call.tool)).toEqual(['tabs_context', 'preview_start'])
  expect(w.state.browser.tabs.size).toBe(1)
})

test('where the browser shows the page in another tab than the one asked for, the next command shows the tab it named', async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  w.state.surfaces = ['desktop']
  w.state.browser.tabs.set('tab-1', PAGE)
  w.state.browser.movesTo = 'seed'
  await openPane($)
  w.state.browser.movesTo = null
  await $.command.run({ command: 'yard-pane', args: '' } as never)
  expect(w.state.browser.calls.filter(call => call.tool === 'navigate').map(call => call.args.tabId)).toEqual(['tab-1', 'seed'])
})

test('a tab of the Yard page the person opened in the browser pane is shown, and the tab it names kept', async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  w.state.surfaces = ['desktop']
  w.state.browser.tabs.set('seed', 'http://127.0.0.1:7411/')
  await openPane($)
  await $.command.run({ command: 'yard-pane', args: '' } as never)
  expect(w.state.browser.calls.filter(call => call.tool !== 'tabs_context')).toEqual([
    { tool: 'navigate', args: { tabId: 'seed', url: PAGE } },
    { tool: 'navigate', args: { tabId: 'seed', url: PAGE } },
  ])
  expect(w.state.browser.tabs.size).toBe(1)
})

test('the terminal, VS Code and the mobile app show the link in the head, and no browser pane is opened for them', async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  w.state.surfaces = ['terminal', 'vscode', 'mobile']
  await openPane($)
  expect(w.state.browser.calls).toEqual([])
  for (const surface of ['terminal', 'vscode', 'mobile'] as const) {
    const ui = await $.ui.mount(paneMount(surface))
    expect((await ui.find({ type: 'Link' }))?.props.href).toBe(PAGE.replace('127.0.0.1', 'localhost'))
    expect(await ui.find({ key: 'browser-unreachable' })).toBeUndefined()
    await ui.unmount()
  }
})

test("in Claude Code Desktop, where the browser pane cannot be reached, the pane shows the link and says so", async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  w.state.surfaces = ['desktop']
  w.state.browser.unreachable = true
  await openPane($)
  const ui = await $.ui.mount(paneMount('desktop'))
  expect((await ui.find({ type: 'Link' }))?.props.href).toBe(PAGE.replace('127.0.0.1', 'localhost'))
  expect((await ui.find({ key: 'browser-unreachable' }))?.text).toBe("Desktop's browser pane could not be reached.")
  await ui.unmount()
  const terminal = await $.ui.mount(paneMount('terminal'))
  expect(await terminal.find({ key: 'browser-unreachable' })).toBeUndefined()
  await terminal.unmount()
})
