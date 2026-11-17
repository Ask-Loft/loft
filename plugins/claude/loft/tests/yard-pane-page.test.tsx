// The Yard pane as the Yard page's companion (the Feature "Keep the Yard page beside the session"):
// the page opened in Claude Code Desktop's browser pane, and in the pane the head, the stage in hand
// and what waits on the person, on every surface Claude Code draws a pane on.

import type { FoundElement } from 'claude-code/testing'
import { expect, test } from 'claude-code/testing'

import { linkOf } from '../hooks/adapter/pane/model'
import { PALETTE } from '../hooks/adapter/pane/theme'
import { answersOf, block, deferred, goaled, openPane, OWNER, PAGE, paneMount, readDoc, staged, start, SURFACES, world, type Engine, type World } from './yard-world'

/** The shades that mark status, each of which must read on a light ground and on a dark one. */
const STATUS_SHADES = ['signal', 'wait', 'fix', 'recommended'] as const

/** Runs `/yard-pane` again, and lets the browser pane's calls it starts settle. */
async function again($: Engine, w: World) {
  await $.command.run({ command: 'yard-pane', args: '' } as never)
  await w.clock.settle()
}


/** The color of the first colored Text an element holds. */
function colorIn(found: { props?: Record<string, unknown>; type?: string; children?: unknown[] } | undefined): unknown {
  if (!found) return undefined
  if (found.type === 'Text' && found.props?.color !== undefined) return found.props.color
  for (const child of found.children ?? []) {
    const color = typeof child === 'object' && child !== null ? colorIn(child as FoundElement) : undefined
    if (color !== undefined) return color
  }
  return undefined
}

/** The contrast of two `#rrggbb` colors, as WCAG measures it. */
function contrast(one: string, other: string) {
  const luminance = (hex: string) => {
    const [r, g, b] = [1, 3, 5].map(at => parseInt(hex.slice(at, at + 2), 16) / 255).map(c => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4))
    return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!
  }
  const [high, low] = [luminance(one), luminance(other)].sort((a, b) => b - a)
  return (high! + 0.05) / (low! + 0.05)
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

test("text, muted text and lines take Claude Code's own colors, and each status shade reads on a light ground and on a dark one", () => {
  expect([PALETTE.ink, PALETTE.muted, PALETTE.line]).toEqual(['text', 'inactive', 'subtle'])
  for (const shade of STATUS_SHADES) {
    expect(contrast(PALETTE[shade], '#ffffff')).toBeGreaterThanOrEqual(3)
    expect(contrast(PALETTE[shade], '#0c1117')).toBeGreaterThanOrEqual(3)
  }
})

test('on every surface the pane draws its text in Claude Code\'s own colors, and no tinted ground but the rust of what waits', async ($, on) => {
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

test('the head counts what waits on the person in the waiting color, and shows no count where nothing waits', async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount(paneMount(surface))
    expect((await ui.find({ key: 'wait-count' }))?.text).toBe('2 wait on you')
    expect((await ui.find({ key: 'wait-count' }))?.props.backgroundColor).toBe(PALETTE.wait)
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
    expect(colorIn(shown)).toBe(PALETTE.signal)
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
    expect(colorIn(await ui.find({ key: 'fix-k-mend' }))).toBe(PALETTE.fix)
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

test('the card of a block that waits is outlined in the waiting color, with a bar of it at its left', async ($, on) => {
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
  expect(answersOf(w).map(each => each.given)).toEqual([[{ block: 'k-readers', labels: ['Visitors'] }]])
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

/** Runs `/yard-pane` once the session has started, and gives the text it answers with. */
async function answered($: Engine) {
  await start($)
  const ran = await $.command.run({ command: 'yard-pane', args: '' } as never)
  return String((ran as { text?: string }).text)
}

/** Holds `/yard-pane`'s answer on `surface`, where no browser pane is reached, to its address and its suggestion. */
async function answersWithAddress($: Engine, w: World, surface: 'terminal' | 'vscode' | 'mobile') {
  w.state.read = LOG
  w.state.surfaces = [surface]
  const text = await answered($)
  expect(text).toContain('http://localhost:7411/#tree=%2Fwork%2Fshelf&branch=feat%2Fshelf')
  expect(text).toContain('The Yard page for feat/shelf shows its Yard Log in full')
  expect(text).toContain('Open it in your browser.')
  expect(text).toContain("Yard Log's pane is open.")
  expect(text).not.toContain('browser pane')
}

test("in the terminal the command answers with the page's address for the tree and branch, what it shows, and to open it", async ($, on) => {
  await answersWithAddress($, world(on), 'terminal')
})

test("in VS Code the command answers with the page's address for the tree and branch, what it shows, and to open it", async ($, on) => {
  await answersWithAddress($, world(on), 'vscode')
})

test("in the mobile app the command answers with the page's address for the tree and branch, what it shows, and to open it", async ($, on) => {
  await answersWithAddress($, world(on), 'mobile')
})

test("in Claude Code Desktop the command's answer says the page opens in Desktop's browser pane", async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  w.state.surfaces = ['desktop']
  const text = await answered($)
  expect(text).toContain(linkOf(PAGE))
  expect(text).toContain("It opens in Desktop's browser pane; if it doesn't, open it in your browser.")
})

test('run while a read is under way, the command waits for that read and answers with the address', async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  await openPane($)
  const held = deferred<void>()
  w.state.reading = held.promise
  await w.clock.advance(1000)
  const ran = $.command.run({ command: 'yard-pane', args: '' } as never)
  held.resolve()
  expect(String(((await ran) as { text?: string }).text)).toContain(linkOf(PAGE))
})

test('where Yard Log cannot be read, the command says so and that the address is not known', async ($, on) => {
  const w = world(on)
  w.state.answers.read = { doc: { outcome: 'error', errors: [{ message: 'HEAD is detached' }] }, exitCode: 1 }
  const text = await answered($)
  expect(text).toContain("Yard Log could not be read, so the Yard page's address is not known.")
  expect(text).not.toContain('http')
})

test('where no page server serves the Yard page, the command says so and gives no address', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ ...LOG, page: { available: false, port: 7411 } })
  const text = await answered($)
  expect(text).toContain('No page server serves the Yard page yet.')
  expect(text).not.toContain('http')
})

test("in Claude Code Desktop the command opens the Yard page at the session's tree and branch in Desktop's browser pane", async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  w.state.surfaces = ['terminal', 'desktop']
  await openPane($)
  await w.clock.settle()
  expect(w.state.browser.calls.map(call => call.tool)).toEqual(['tabs_context', 'preview_start'])
  expect(w.state.browser.calls[1]!.args.url).toBe(PAGE)
  expect([...w.state.browser.tabs.values()]).toEqual([PAGE])
})

test("where Desktop's browser pane already shows the Yard page, the command shows that tab and opens no other", async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  w.state.surfaces = ['desktop']
  await openPane($)
  await w.clock.settle()
  await again($, w)
  expect(w.state.browser.calls.map(call => call.tool)).toEqual(['tabs_context', 'preview_start', 'tabs_context', 'navigate'])
  expect(w.state.browser.calls[3]!.args).toEqual({ tabId: 'tab-1', url: PAGE })
  expect(w.state.browser.tabs.size).toBe(1)
  w.state.browser.tabs.clear()
  await again($, w)
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
  await w.clock.settle()
  w.state.browser.movesTo = null
  // Another tab of the page server, listed before the one the browser named.
  const named = w.state.browser.tabs.get('seed')!
  w.state.browser.tabs.clear()
  w.state.browser.tabs.set('other', 'http://127.0.0.1:7411/')
  w.state.browser.tabs.set('seed', named)
  await again($, w)
  expect(w.state.browser.calls.filter(call => call.tool === 'navigate').map(call => call.args.tabId)).toEqual(['tab-1', 'seed'])
})

test('a tab of the Yard page opened from its localhost link is shown, not a second one opened', async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  w.state.surfaces = ['desktop']
  w.state.browser.tabs.set('seed', 'http://localhost:7411/')
  await openPane($)
  await w.clock.settle()
  expect(w.state.browser.calls.map(call => [call.tool, call.args.tabId])).toEqual([['tabs_context', undefined], ['navigate', 'seed']])
})

test('the tab the command left, once it shows another site, is left alone and the page opened in a new one', async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  w.state.surfaces = ['desktop']
  await openPane($)
  await w.clock.settle()
  w.state.browser.tabs.set('tab-1', 'https://docs.example/')
  await again($, w)
  expect(w.state.browser.calls.slice(2).map(call => call.tool)).toEqual(['tabs_context', 'preview_start'])
})

test('a browser pane that could not be reached once keeps the tab the command left, for the next command', async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  w.state.surfaces = ['desktop']
  await openPane($)
  await w.clock.settle()
  w.state.browser.unreachable = true
  await again($, w)
  w.state.browser.unreachable = false
  const left = w.state.browser.tabs.get('tab-1')!
  w.state.browser.tabs.clear()
  w.state.browser.tabs.set('other', 'http://127.0.0.1:7411/')
  w.state.browser.tabs.set('tab-1', left)
  await again($, w)
  expect(w.state.browser.calls.at(-1)).toEqual({ tool: 'navigate', args: { tabId: 'tab-1', url: PAGE } })
})

test('words before the JSON of a browser answer do not hide the tabs it lists', async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  w.state.surfaces = ['desktop']
  w.state.browser.preface = 'Tabs of the browser pane:\n'
  w.state.browser.tabs.set('seed', 'http://127.0.0.1:7411/')
  await openPane($)
  await w.clock.settle()
  expect(w.state.browser.calls.map(call => call.tool)).toEqual(['tabs_context', 'navigate'])
})

test('run while a read is under way, the command still shows the page once that read ends', async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  w.state.surfaces = ['desktop']
  await openPane($)
  await w.clock.settle()
  const held = deferred<void>()
  w.state.reading = held.promise
  await w.clock.advance(1000)
  const ran = $.command.run({ command: 'yard-pane', args: '' } as never)
  w.state.reading = Promise.resolve()
  held.resolve()
  await ran
  await w.clock.settle()
  expect(w.state.browser.calls.slice(2).map(call => call.tool)).toEqual(['tabs_context', 'navigate'])
})

test('the command answers without waiting on a browser pane that does not answer', async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  w.state.surfaces = ['desktop']
  w.state.browser.hangs = 'preview_start'
  await openPane($)
  const ran = await $.command.run({ command: 'yard-pane', args: '' } as never)
  expect(ran).toBeDefined()
})

test('a tab whose title holds quotes and braces is still found among the browser pane\'s tabs, and shown', async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  w.state.surfaces = ['desktop']
  w.state.browser.tabs.set('seed', 'http://127.0.0.1:7411/')
  w.state.browser.title = 'The "}" page'
  await openPane($)
  await w.clock.settle()
  expect(w.state.browser.calls.map(call => call.tool)).toEqual(['tabs_context', 'navigate'])
  expect(w.state.browser.tabs.size).toBe(1)
})

test('a tab of the Yard page the person opened in the browser pane is shown, and the tab it names kept', async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  w.state.surfaces = ['desktop']
  w.state.browser.tabs.set('seed', 'http://127.0.0.1:7411/')
  await openPane($)
  await w.clock.settle()
  await again($, w)
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
  await w.clock.settle()
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
  await w.clock.settle()
  const ui = await $.ui.mount(paneMount('desktop'))
  expect((await ui.find({ type: 'Link' }))?.props.href).toBe(PAGE.replace('127.0.0.1', 'localhost'))
  expect((await ui.find({ key: 'browser-unreachable' }))?.text).toBe("Desktop's browser pane could not be reached.")
  await ui.unmount()
  const terminal = await $.ui.mount(paneMount('terminal'))
  expect(await terminal.find({ key: 'browser-unreachable' })).toBeUndefined()
  await terminal.unmount()
})
