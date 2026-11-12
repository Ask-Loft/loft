// The Yard pane drawn as the Yard page draws Yard Log (the Feature "See Yard Log in the pane as on
// the Yard page"), on every surface Claude Code draws a pane on.

import type { FoundElement } from 'claude-code/testing'
import { expect, test } from 'claude-code/testing'

import { clockOf } from '../hooks/adapter/pane/model'
import { PALETTE, STATUS_SHADES } from '../hooks/adapter/pane/theme'
import { block, openPane, PAGE, paneMount, readDoc, SESSION, stage, world, type Engine } from './yard-world'

const SURFACES = ['terminal', 'desktop', 'vscode', 'mobile'] as const

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

/** The contrast of two `#rrggbb` colours, as WCAG measures it. */
function contrast(one: string, other: string) {
  const luminance = (hex: string) => {
    const [r, g, b] = [1, 3, 5].map(at => parseInt(hex.slice(at, at + 2), 16) / 255).map(c => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4))
    return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!
  }
  const [high, low] = [luminance(one), luminance(other)].sort((a, b) => b - a)
  return (high! + 0.05) / (low! + 0.05)
}

test("text, muted text and lines take Claude Code's own colours, and each status shade reads on a light ground and on a dark one", () => {
  expect([PALETTE.ink, PALETTE.body, PALETTE.muted, PALETTE.line]).toEqual(['text', 'text', 'inactive', 'subtle'])
  for (const shade of STATUS_SHADES) {
    expect(contrast(PALETTE[shade], '#ffffff')).toBeGreaterThanOrEqual(3)
    expect(contrast(PALETTE[shade], '#0c1117')).toBeGreaterThanOrEqual(3)
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

test('the head counts what waits on the person in a button of the waiting colour, and shows none where nothing waits', async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount(paneMount(surface))
    expect((await ui.find({ key: 'wait-count' }))?.text).toBe('2 wait on you')
    expect((await ui.find({ key: 'wait-control' }))?.props.backgroundColor).toBe(PALETTE.wait)
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

test('on every surface the pane draws its text in Claude Code\'s own colours, and no tinted ground but the rust of what waits', async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount(paneMount(surface))
    expect((await ui.find({ type: 'Text', text: 'feat/shelf' }))?.props.color).toBe('text')
    expect((await ui.find({ type: 'Text', text: 'shelf' }))?.props.color).toBe('inactive')
    await ui.press({ key: 'tab-timeline' })
    const grounds = (await ui.findAll({})).flatMap(each => (each.props.backgroundColor === undefined ? [] : [each.props.backgroundColor]))
    expect(new Set(grounds)).toEqual(new Set([PALETTE.wait]))
    await ui.unmount()
  }
})

/** The pane mounted on `surface`, `columns` wide, showing the timeline. */
async function timelineOf($: Engine, surface: Parameters<typeof paneMount>[0], columns?: number) {
  const ui = await $.ui.mount(paneMount(surface, columns))
  await ui.press({ key: 'tab-timeline' })
  return ui
}

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
    const ui = await timelineOf($, surface)
    expect((await ui.findAll({ type: 'Text', text: '■' })).map(each => each.props.color)).toEqual([PALETTE.done, PALETTE.signal, PALETTE.rail])
    expect((await ui.find({ key: 'choose-k-build' }))?.text).toBe('Build the store')
    expect((await ui.find({ type: 'Text', text: 'Done' }))?.props.color).toBe(PALETTE.done)
    expect((await ui.find({ type: 'Text', text: 'Active' }))?.props.color).toBe(PALETTE.signal)
    expect((await ui.find({ type: 'Text', text: /^Planned$/ }))?.props.color).toBe(PALETTE.rail)
    expect(await ui.findAll({ type: 'Text', text: 'main agent' })).not.toHaveLength(0)
    expect((await ui.find({ type: 'Text', text: '1 waiting' }))?.props.color).toBe(PALETTE.wait)
    await ui.unmount()
  }
})

test('a line marks where the planned stages begin', async ($, on) => {
  const w = world(on)
  w.state.read = RAIL
  await openPane($)
  const ui = await timelineOf($, 'desktop')
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
    const ui = await timelineOf($, surface)
    expect(await ui.find({ key: 'block-row-k-store' })).toBeDefined()
    expect((await ui.find({ key: 'block-state-k-store' }))?.text).toBe('Active')
    expect((await ui.find({ key: 'block-state-k-go' }))?.text).toBe('Waits on you')
    expect(colourIn(await ui.find({ key: 'block-state-k-go' }))).toBe(PALETTE.wait)
    expect(colourIn(await ui.find({ key: 'chosen-k-build' }))).toBe(PALETTE.signal)
    expect(await ui.find({ key: 'chosen-k-shape' })).toBeUndefined()
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
  const ui = await timelineOf($, 'desktop')
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
  const ui = await timelineOf($, 'terminal')
  expect((await ui.find({ key: 'stage-title' }))?.text).toBe('Stage shape')
  await ui.unmount()
})

test('from 100 columns the rail and the stage chosen stand side by side, and below that one above the other', async ($, on) => {
  const w = world(on)
  w.state.read = RAIL
  await openPane($)
  for (const surface of SURFACES) {
    const wide = await timelineOf($, surface, 120)
    expect((await wide.find({ key: 'timeline' }))?.props.flexDirection).toBe('row')
    await wide.unmount()
    const narrow = await timelineOf($, surface, 60)
    expect((await narrow.find({ key: 'timeline' }))?.props.flexDirection).toBe('column')
    await narrow.unmount()
  }
})

const v = (at: string, number: number, by: Record<string, unknown> = OWNER) => ({ act: { at, by, revision: 1 }, number })

/** A block as a whole read with `--versions` gives it, its versions written at `ats`. */
function written(id: string, kind: string, body: Record<string, unknown>, ats = ['2026-10-22T09:00:00Z']) {
  const versions = ats.map((at, n) => ({ ...v(at, n + 1), body }))
  return { ...block(id, kind, body), version: versions.at(-1)!, versions, versionCount: versions.length }
}

const READERS = { prompt: 'Which readers does the change serve?', header: 'Readers', context: 'Cards are **new**.', options: [{ label: 'Members', detail: 'Readers who hold a card', recommended: true }, { label: 'Visitors', detail: 'Anyone at the desk' }] }

const CARDS = readDoc({
  stages: [
    {
      ...staged(
        'build',
        'active',
        written('readers', 'question', READERS),
        written('go', 'approval', { title: 'Approve the store', markdown: 'The shelf holds **500** books.' }),
        written('store', 'task', { title: 'Write the store', state: 'active', detail: 'In one file.' }, ['2026-10-22T09:00:00Z', '2026-10-22T09:20:00Z']),
        written('mend', 'task', { title: 'Mend the reload', state: 'open', type: 'fix' }),
        written('result', 'note', { title: 'What the store holds', markdown: 'Every **book** on the shelf.' }),
        written('choice', 'decision', { title: 'Members only', consequence: 'Visitors wait for the next change' }),
        written('change', 'loft', { view: 'spec-change', note: 'Two Requirements changed.', report: { text: 'THE WHOLE REPORT' } }),
      ),
    },
  ],
  submissions: [{ agent: OWNER, open: [{ key: 's-1', state: 'open', blocks: ['k-readers', 'k-go'] }] }],
})

const SETTLED = readDoc({
  stages: [
    staged(
      'shape',
      'active',
      { ...written('readers', 'question', READERS), answer: { labels: ['Members'], words: 'and staff', channel: 'page' } },
      { ...written('go', 'approval', { title: 'Approve the store', markdown: 'All.' }), outcome: { verdict: 'approved', channel: 'terminal' } },
      { ...written('no', 'approval', { title: 'Approve the reload', markdown: 'All.' }), outcome: { verdict: 'rejected', reason: 'Too wide', channel: 'page' } },
      { ...written('nay', 'plan', { title: 'The plan', markdown: 'Steps.' }), outcome: { verdict: 'rejected', channel: 'page' } },
    ),
  ],
})

/** The text an element description holds: its strings, and its children's, in order. */
type Described = { children?: unknown[] }
const textIn = (node: unknown): string =>
  typeof node === 'string' ? node : typeof node === 'object' && node !== null ? ((node as Described).children ?? []).map(textIn).join('') : ''

/** The texts of the elements an element holds, each apart. */
const partsOf = (found: FoundElement | undefined) => (found?.children ?? []).map(textIn)

test("a card's head names the block's kind, a question's header as a chip, and the block's state at the right", async ($, on) => {
  const w = world(on)
  w.state.read = CARDS
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await timelineOf($, surface)
    const head = await ui.find({ key: 'card-head-k-store' })
    expect(head?.props.justifyContent).toBe('space-between')
    expect(head?.text).toMatch(/^Task.*Active$/)
    expect((await ui.find({ key: 'card-head-k-readers' }))?.text).toMatch(/^Question.*Readers.*Waits on you$/)
    expect((await ui.find({ key: 'chip-k-readers' }))?.props.borderColor).toBe(PALETTE.line)
    expect((await ui.find({ key: 'card-head-k-result' }))?.text).toBe('Note')
    await ui.unmount()
  }
})

test('the card of a block that waits on the person stands on the waiting ground with a bar of the waiting colour, and says so', async ($, on) => {
  const w = world(on)
  w.state.read = CARDS
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await timelineOf($, surface)
    for (const key of ['k-readers', 'k-go']) {
      expect((await ui.find({ key: `card-${key}` }))?.props.borderColor).toBe(PALETTE.wait)
      expect((await ui.find({ key: `card-${key}` }))?.props.backgroundColor).toBeUndefined()
      expect((await ui.find({ key: `wait-bar-${key}` }))?.props.backgroundColor).toBe(PALETTE.wait)
    }
    expect((await ui.find({ key: 'card-k-store' }))?.props.borderColor).toBe(PALETTE.line)
    expect(await ui.find({ key: 'wait-bar-k-store' })).toBeUndefined()
    expect(await ui.findAll({ type: 'Text', text: 'This waits on you.' })).toHaveLength(2)
    expect((await ui.find({ key: 'answer-it-k-readers' }))?.props.label).toBe('Answer it')
    expect((await ui.find({ key: 'decide-it-k-go' }))?.props.label).toBe('Decide it')
    await ui.unmount()
  }
})

test('a question shows its prompt, its context as Markdown and each option as a card lettered A, B and on, the recommended one marked', async ($, on) => {
  const w = world(on)
  w.state.read = CARDS
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await timelineOf($, surface)
    expect(await ui.find({ type: 'Text', text: 'Which readers does the change serve?' })).toBeDefined()
    expect(await ui.find({ type: 'Markdown', text: 'Cards are **new**.' })).toBeDefined()
    expect((await ui.find({ key: 'card-option-k-readers-0' }))?.text).toMatch(/^A.*Members.*Recommended.*Readers who hold a card$/)
    expect((await ui.find({ key: 'card-option-k-readers-1' }))?.text).toMatch(/^B.*Visitors.*Anyone at the desk$/)
    expect((await ui.find({ key: 'card-option-k-readers-1' }))?.text).not.toContain('Recommended')
    expect((await ui.find({ key: 'card-option-k-readers-0' }))?.props.borderStyle).toBe('single')
    await ui.unmount()
  }
})

test("an answered question shows the options chosen and the person's own words, and where it was answered", async ($, on) => {
  const w = world(on)
  w.state.read = SETTLED
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await timelineOf($, surface)
    expect(partsOf(await ui.find({ key: 'answer-k-readers' }))).toEqual(['Members, “and staff”', 'answered on the page'])
    expect((await ui.find({ key: 'card-option-k-readers-0' }))?.props.borderColor).toBe(PALETTE.signal)
    expect((await ui.find({ key: 'card-option-k-readers-1' }))?.props.borderColor).toBe(PALETTE.line)
    expect((await ui.find({ key: 'card-option-k-readers-2' }))?.text).toMatch(/^C.*In your own words.*and staff$/)
    expect((await ui.find({ key: 'card-head-k-readers' }))?.text).toMatch(/Answered$/)
    await ui.unmount()
  }
})

test('an approval or a plan shows its title and its Markdown, and once decided, Approved, or Rejected with the reason', async ($, on) => {
  const w = world(on)
  w.state.read = SETTLED
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await timelineOf($, surface)
    expect(await ui.find({ type: 'Text', text: 'Approve the store' })).toBeDefined()
    expect(await ui.find({ type: 'Markdown', text: 'All.' })).toBeDefined()
    expect((await ui.find({ key: 'outcome-k-go' }))?.text).toBe('Approved in the terminal')
    expect(colourIn(await ui.find({ key: 'outcome-k-go' }))).toBe(PALETTE.done)
    expect((await ui.find({ key: 'outcome-k-no' }))?.text).toBe('Rejected on the page: Too wide')
    expect(colourIn(await ui.find({ key: 'outcome-k-no' }))).toBe(PALETTE.danger)
    expect((await ui.find({ key: 'outcome-k-nay' }))?.text).toBe('Rejected on the page, with no reason given')
    await ui.unmount()
  }
  w.state.read = CARDS
  await $.command.run({ command: 'yard-pane', args: '' } as never)
  const ui = await timelineOf($, 'desktop')
  expect(await ui.find({ type: 'Markdown', text: 'The shelf holds **500** books.' })).toBeDefined()
  expect(await ui.find({ key: 'outcome-k-go' })).toBeUndefined()
  await ui.unmount()
})

test('a task shows its title and its state, and a fix is marked Fix in the fix colour', async ($, on) => {
  const w = world(on)
  w.state.read = CARDS
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await timelineOf($, surface)
    expect((await ui.find({ key: 'card-k-store' }))?.text).toMatch(/Write the store/)
    expect((await ui.find({ key: 'card-head-k-store' }))?.text).toMatch(/Active$/)
    expect(await ui.find({ type: 'Markdown', text: 'In one file.' })).toBeDefined()
    expect((await ui.find({ key: 'card-head-k-mend' }))?.text).toMatch(/^Task.*Fix.*Open$/)
    expect((await ui.find({ key: 'fix-k-mend' }))?.props.backgroundColor).toBeUndefined()
    expect(colourIn(await ui.find({ key: 'fix-k-mend' }))).toBe(PALETTE.fix)
    expect(await ui.find({ key: 'fix-k-store' })).toBeUndefined()
    await ui.unmount()
  }
})

test("a note shows its Markdown, a decision its consequence, and a loft block its view in words and its note, and no report", async ($, on) => {
  const w = world(on)
  w.state.read = CARDS
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await timelineOf($, surface)
    expect((await ui.find({ key: 'card-k-result' }))?.text).toMatch(/What the store holds/)
    expect(await ui.find({ type: 'Markdown', text: 'Every **book** on the shelf.' })).toBeDefined()
    expect((await ui.find({ key: 'card-k-choice' }))?.text).toMatch(/Members only.*Visitors wait for the next change/)
    expect((await ui.find({ key: 'card-k-change' }))?.text).toMatch(/^Loft.*Spec change.*Two Requirements changed\./)
    expect(await ui.find({ type: 'Markdown', text: 'Two Requirements changed.' })).toBeDefined()
    expect(await ui.find({ text: /THE WHOLE REPORT/ })).toBeUndefined()
    expect(await ui.find({ text: /spec-change/ })).toBeUndefined()
    await ui.unmount()
  }
})

test("a card's foot names who wrote the block, its version, and when it was created and last updated", async ($, on) => {
  const w = world(on)
  w.state.read = CARDS
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await timelineOf($, surface)
    expect(partsOf(await ui.find({ key: 'foot-k-store' }))).toEqual(['main agent', 'version 2', `Created ${clockOf('2026-10-22T09:00:00Z')}`, `Updated ${clockOf('2026-10-22T09:20:00Z')}`])
    expect(partsOf(await ui.find({ key: 'foot-k-mend' }))).toEqual(['main agent', 'version 1', `Created ${clockOf('2026-10-22T09:00:00Z')}`])
    await ui.unmount()
  }
})

const recorded = (w: ReturnType<typeof world>) => w.state.runs.filter(run => run.argv.includes('answer')).map(run => JSON.parse(run.stdin ?? '[]'))

/** `log` with one more open submission, `key`, of the approval `go` of stage `build`, at `revision`. */
function asking(revision: number, key = 's-2') {
  return readDoc({
    revision,
    stages: [
      ...(LOG.stages as object[]).slice(0, 1),
      staged('build', 'active', block('store', 'task', { title: 'Write the store', state: 'active' }), block('go', 'approval', { title: 'Approve the store', markdown: 'All.' })),
      ...(LOG.stages as object[]).slice(2),
    ],
    submissions: [{ agent: OWNER, open: [{ key, state: 'open', blocks: ['k-go'] }] }],
  })
}

test('pressing Answer it or Decide it on a waiting card shows what waits, at that block', async ($, on) => {
  const w = world(on)
  w.state.read = CARDS
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount(paneMount(surface))
    await ui.press({ key: 'tab-timeline' })
    expect(await ui.find({ key: 'view-waiting' })).toBeUndefined()
    await ui.press({ key: 'decide-it-k-go' })
    expect(await ui.find({ key: 'view-waiting' })).toBeDefined()
    expect((await ui.find({ key: 'ask-k-go' }))?.props.borderColor).toBe(PALETTE.signal)
    expect((await ui.find({ key: 'ask-k-readers' }))?.props.borderColor).toBe(PALETTE.wait)
    await ui.press({ key: 'tab-timeline' })
    await ui.press({ key: 'answer-it-k-readers' })
    expect((await ui.find({ key: 'ask-k-readers' }))?.props.borderColor).toBe(PALETTE.signal)
    await ui.unmount()
  }
})

test('the button that counts what waits shows it, the oldest submission first', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({
    stages: (CARDS.stages as object[]),
    submissions: [
      { agent: OWNER, open: [{ key: 's-new', state: 'open', blocks: ['k-readers'], sent: { at: '2026-10-22T10:00:00Z' } }] },
      { agent: { ...OWNER, agent: 'sub-1' }, open: [{ key: 's-old', state: 'open', blocks: ['k-go'], sent: { at: '2026-10-22T09:00:00Z' } }] },
    ],
  })
  await openPane($)
  const ui = await $.ui.mount(paneMount('desktop'))
  await ui.press({ key: 'tab-timeline' })
  await ui.press({ key: 'wait-count' })
  const asks = (await ui.findAll({})).flatMap(each => (each.key?.startsWith('ask-') ? [each.key] : []))
  expect(asks).toEqual(['ask-k-go', 'ask-k-readers'])
  expect((await ui.find({ key: 'waiting-title' }))?.text).toBe('2 blocks wait on you')
  await ui.unmount()
})

test('what waits shows by itself when a submission arrives that the pane has not shown, and not again once left', async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  await openPane($)
  const ui = await $.ui.mount(paneMount('desktop'))
  expect(await ui.find({ key: 'view-timeline' })).toBeDefined()
  w.state.read = asking(5)
  await w.clock.advance(1000)
  expect(await ui.find({ key: 'view-waiting' })).toBeDefined()
  await ui.press({ key: 'tab-timeline' })
  w.state.read = asking(6)
  await w.clock.advance(1000)
  expect(await ui.find({ key: 'view-timeline' })).toBeDefined()
  w.state.read = asking(7, 's-3')
  await w.clock.advance(1000)
  expect(await ui.find({ key: 'view-waiting' })).toBeDefined()
  await ui.unmount()
})

test('once nothing waits on the person, the pane shows again the view it showed before', async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  await openPane($)
  const ui = await $.ui.mount(paneMount('desktop'))
  await ui.press({ key: 'tab-gantt' })
  w.state.read = asking(5)
  await w.clock.advance(1000)
  expect(await ui.find({ key: 'view-waiting' })).toBeDefined()
  expect((await ui.find({ key: 'tab-gantt' }))?.props.variant).toBe('secondary')
  w.state.read = { ...asking(6), submissions: [] }
  await w.clock.advance(1000)
  expect(await ui.find({ key: 'view-gantt' })).toBeDefined()
  expect((await ui.find({ key: 'tab-gantt' }))?.props.variant).toBe('primary')
  await ui.unmount()
})

test("a question's lettered option cards choose its options, and a field beneath them takes the person's own words", async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  await openPane($)
  const ui = await $.ui.mount(paneMount('desktop'))
  expect(await ui.find({ key: 'view-waiting' })).toBeDefined()
  expect((await ui.find({ key: 'pick-k-readers-0' }))?.text).toMatch(/^A.*Members.*Recommended/)
  expect((await ui.find({ key: 'pick-k-readers-1' }))?.text).toMatch(/^B.*Visitors/)
  const keys = (await ui.findAll({})).map(each => each.key)
  expect(keys.indexOf('words-k-readers')).toBeGreaterThan(keys.indexOf('pick-k-readers-1'))
  await ui.press({ key: 'option-k-readers-1' })
  expect(recorded(w)).toEqual([[{ block: 'k-readers', labels: ['Visitors'] }]])
  await ui.unmount()
})

const TIMED = readDoc({
  stages: [
    { ...staged('shape', 'done'), times: { started: '2026-10-22T08:00:00Z', ended: '2026-10-22T08:30:00Z' } },
    {
      ...staged('build', 'active', { ...block('store', 'task', { title: 'Write the store', state: 'done' }), times: { started: '2026-10-22T08:45:00Z', ended: '2026-10-22T09:10:00Z' } } as ReturnType<typeof block>),
      times: { started: '2026-10-22T08:40:00Z' },
    },
    staged('review', 'planned'),
  ],
})

test('Claude Code Desktop, VS Code and the mobile app draw the Gantt chart as one SVG image, named in words for who cannot see it', async ($, on) => {
  const w = world(on)
  w.state.read = TIMED
  await openPane($)
  for (const surface of ['desktop', 'vscode', 'mobile'] as const) {
    const ui = await $.ui.mount(paneMount(surface))
    await ui.press({ key: 'tab-gantt' })
    const images = await ui.findAll({ type: 'Svg' })
    expect(images).toHaveLength(1)
    expect(String(images[0]!.props.source)).toMatch(/^<svg /)
    expect(String(images[0]!.props.source)).toContain(PALETTE.done)
    expect(images[0]!.props.alt).toContain('Stage shape ran 30 min.')
    expect(images[0]!.props.alt).toMatch(/Stage build has run .*, and goes on\./)
    expect(await ui.find({ key: 'gantt-bar-k-shape' })).toBeUndefined()
    await ui.unmount()
  }
})

test('the terminal draws the Gantt chart as a bar of block characters for each row, in its colour', async ($, on) => {
  const w = world(on)
  w.state.read = TIMED
  await openPane($)
  const ui = await $.ui.mount(paneMount('terminal', 80))
  await ui.press({ key: 'tab-gantt' })
  expect(await ui.find({ type: 'Svg' })).toBeUndefined()
  expect((await ui.find({ key: 'gantt-bar-k-shape' }))?.text).toMatch(/^ *█+ *$/)
  expect(colourIn(await ui.find({ key: 'gantt-bar-k-shape' }))).toBe(PALETTE.done)
  expect((await ui.find({ key: 'gantt-bar-k-store' }))?.text).toMatch(/^ *▬+ *$/)
  expect(colourIn(await ui.find({ key: 'gantt-bar-k-store' }))).toBe(PALETTE.task)
  expect(await ui.find({ key: 'gantt-bar-k-review' })).toBeUndefined()
  await ui.unmount()
})

test('choosing a stage in the Gantt chart shows it, chosen, in the timeline', async ($, on) => {
  const w = world(on)
  w.state.read = TIMED
  await openPane($)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount(paneMount(surface))
    await ui.press({ key: 'tab-gantt' })
    await ui.press({ key: 'gantt-stage-k-shape' })
    expect(await ui.find({ key: 'view-timeline' })).toBeDefined()
    expect((await ui.find({ key: 'stage-title' }))?.text).toBe('Stage shape')
    expect(await ui.find({ key: 'chosen-k-shape' })).toBeDefined()
    await ui.press({ key: 'choose-k-build' })
    await ui.unmount()
  }
})
