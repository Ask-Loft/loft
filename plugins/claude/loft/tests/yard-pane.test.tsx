import { expect, test } from 'claude-code/testing'

import { block, PAGE, readDoc, stage, start, world, yardRuns, type Engine, type World } from './yard-world'

const OWNER = { host: 'claude-code', session: 'session-1' }

/** The `loft yard read` runs, as the words after `yard` but the port and the format every run names. */
const reads = (w: World) =>
  yardRuns(w)
    .filter(run => run.startsWith('read'))
    .map(run => run.replace(' --port 7411 --format json', ''))

async function opened($: Engine, w: World) {
  await start($)
  await $.command.run({ command: 'yard-pane', args: '' } as never)
  return w
}

test('the command /yard-pane is offered once Yard Client is on, and runs while a turn runs', async ($, on) => {
  const w = world(on)
  await start($)
  const command = w.state.commands.find(each => each.name === 'yard-pane')
  expect(command?.immediate).toBe(true)
  expect(typeof command?.description).toBe('string')
})

test('the command /yard-pane is not offered while Yard Client is off', async ($, on) => {
  const w = world(on, { protocol: 'none' })
  await start($)
  expect(w.state.commands).toEqual([])
})

test('the command opens the pane and reads Yard Log whole, for the session, marking nothing read', async ($, on) => {
  const w = await opened($, world(on))
  expect(w.state.panes).toEqual(['yard'])
  expect(reads(w).at(-1)).toBe('read --session session-1')
})

test('running the command again shows the same pane, read whole again, and keeps one follow going', async ($, on) => {
  const w = await opened($, world(on))
  await $.command.run({ command: 'yard-pane', args: '' } as never)
  expect(w.state.opened.map(each => each.id)).toEqual(['yard', 'yard'])
  expect(w.state.panes).toEqual(['yard'])
  const before = reads(w).length
  await w.clock.advance(1000)
  expect(reads(w).length).toBe(before + 1)
})

test('while the pane is open, each read after the first asks only for what changed since the revision shown', async ($, on) => {
  const w = await opened($, world(on))
  w.state.read = readDoc({
    revision: 6,
    since: 4,
    stages: [{ ...stage('build', 'active', OWNER), blocks: [block('store', 'task', { title: 'The store', state: 'active' })] }],
  })
  await w.clock.advance(1000)
  expect(reads(w).at(-1)).toBe('read --session session-1 --since 4')
  await w.clock.advance(1000)
  expect(reads(w).at(-1)).toBe('read --session session-1 --since 6')
  expect(yardRuns(w).some(run => run.includes('--mark-read'))).toBe(false)
})

test('a checkout of another branch is read whole at once', async ($, on) => {
  const w = await opened($, world(on))
  w.state.read = readDoc({ revision: 2, since: 4, branch: 'feat/other' })
  await w.clock.advance(1000)
  expect(reads(w).slice(-2)).toEqual(['read --session session-1 --since 4', 'read --session session-1'])
})

test('once the pane is closed, the mod reads nothing for it', async ($, on) => {
  const w = await opened($, world(on))
  w.state.panes = []
  const before = reads(w).length
  await w.clock.advance(5000)
  expect(reads(w).length).toBe(before)
})

const SURFACES = ['terminal', 'desktop', 'vscode', 'mobile'] as const

const PANE = {
  plugin: 'loft',
  component: 'Pane',
  requestId: 'yard',
  props: { title: 'Yard', isFocused: false, bodyColumns: 80, placement: 'dock', scroll: { offset: 0, bodyRows: 40 }, view: {} },
} as const

/** Mounts the pane on `surface`, as Claude Code draws it there. */
const mount = ($: Engine, surface: (typeof SURFACES)[number]) => $.ui.mount({ ...PANE, surface } as never)

function staged(id: string, status: string, goal: string, ...blocks: ReturnType<typeof block>[]) {
  const held = stage(id, status, OWNER)
  return { ...held, version: { body: { ...held.version.body, title: `Stage ${id}`, goal } }, blocks }
}

const LOG = readDoc({
  stages: [
    staged('shape', 'done', 'The change is shaped', block('choice', 'decision', { title: 'Members only', consequence: 'Visitors wait for the next change' })),
    staged(
      'build',
      'active',
      'The store holds the shelf',
      block('store', 'task', { title: 'Write the store', state: 'active', type: 'regular' }),
      block('mend', 'task', { title: 'Mend the reload', state: 'open', type: 'fix' }),
      block('result', 'note', { title: 'What the store holds', markdown: 'Every **book** on the shelf.' }),
      block('change', 'loft', { view: 'spec-change', note: 'Two Requirements changed.', report: { text: 'THE WHOLE REPORT' } }),
    ),
    staged('review', 'planned', 'The change is reviewed', block('later', 'note', { markdown: 'Planned work.' })),
  ],
})

test('the header names the branch, the revision shown and the Yard page', async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  await opened($, w)
  for (const surface of SURFACES) {
    const ui = await mount($, surface)
    expect((await ui.find({ type: 'Text', text: /feat\/shelf/ }))?.text).toContain('revision 4')
    expect((await ui.find({ type: 'Link' }))?.props.href).toBe(PAGE.replace('127.0.0.1', 'localhost'))
    await ui.unmount()
  }
})

test('each stage shows its title, status and goal, and its tasks their title and state, a fix marked', async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  await opened($, w)
  for (const surface of SURFACES) {
    const ui = await mount($, surface)
    expect((await ui.find({ type: 'Text', text: /Stage build/ }))?.text).toContain('active')
    expect(await ui.find({ type: 'Text', text: 'The store holds the shelf' })).toBeDefined()
    expect((await ui.find({ type: 'Text', text: /Write the store/ }))?.text).toContain('active')
    expect((await ui.find({ type: 'Text', text: /Write the store/ }))?.text).not.toContain('fix')
    expect((await ui.find({ type: 'Text', text: /Mend the reload/ }))?.text).toContain('open')
    expect((await ui.find({ type: 'Text', text: /Mend the reload/ }))?.text).toContain('fix')
    await ui.unmount()
  }
})

test("a note's Markdown, a decision's consequence and a loft block's view and note are drawn, and no report", async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  await opened($, w)
  for (const surface of SURFACES) {
    const ui = await mount($, surface)
    await ui.press({ key: 'unfold-k-shape' })
    expect(await ui.find({ type: 'Text', text: 'What the store holds' })).toBeDefined()
    expect((await ui.find({ type: 'Markdown', text: /book/ }))?.text).toBe('Every **book** on the shelf.')
    expect(await ui.find({ type: 'Text', text: /Members only/ })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: 'Visitors wait for the next change' })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /spec-change/ })).toBeDefined()
    expect(await ui.find({ type: 'Markdown', text: 'Two Requirements changed.' })).toBeDefined()
    expect(await ui.find({ text: /THE WHOLE REPORT/ })).toBeUndefined()
    await ui.press({ key: 'unfold-k-shape' })
    await ui.unmount()
  }
})

test('a stage done or dropped shows its title and status alone until unfolded, and a planned stage its blocks', async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  await opened($, w)
  for (const surface of SURFACES) {
    const ui = await mount($, surface)
    expect((await ui.find({ type: 'Text', text: /Stage shape/ }))?.text).toContain('done')
    expect(await ui.find({ type: 'Text', text: 'The change is shaped' })).toBeUndefined()
    expect(await ui.find({ type: 'Text', text: /Members only/ })).toBeUndefined()
    expect(await ui.find({ type: 'Markdown', text: 'Planned work.' })).toBeDefined()
    await ui.press({ key: 'unfold-k-shape' })
    expect(await ui.find({ type: 'Text', text: /Members only/ })).toBeDefined()
    await ui.press({ key: 'unfold-k-shape' })
    expect(await ui.find({ type: 'Text', text: /Members only/ })).toBeUndefined()
    await ui.unmount()
  }
})

const ASKING = readDoc({
  stages: [
    staged(
      'shape',
      'active',
      'Shape it',
      block('readers', 'question', {
        prompt: 'Which readers does the change serve?',
        header: 'Readers',
        context: 'Cards are **new**.',
        multiple: false,
        options: [
          { label: 'Members', detail: 'Readers who hold a card', recommended: true },
          { label: 'Visitors', detail: 'Anyone at the desk', recommended: false },
        ],
      }),
      block('go', 'approval', { title: 'Approve the shape', markdown: 'The shelf holds **500** books.' }),
    ),
  ],
  submissions: [{ agent: OWNER, open: [{ key: 's-1', state: 'open', blocks: ['k-readers', 'k-go'] }] }],
})

test('what waits on the person shows above the stages: questions with their options, approvals with their Markdown', async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  await opened($, w)
  for (const surface of SURFACES) {
    const ui = await mount($, surface)
    expect(await ui.find({ key: 'waiting' })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: 'Readers' })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: 'Which readers does the change serve?' })).toBeDefined()
    expect(await ui.find({ type: 'Markdown', text: 'Cards are **new**.' })).toBeDefined()
    expect((await ui.find({ key: 'option-k-readers-0' }))?.text).toBe('Members (recommended)')
    expect((await ui.find({ key: 'option-k-readers-1' }))?.text).toBe('Visitors')
    expect(await ui.find({ type: 'Text', text: 'Readers who hold a card' })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: 'Approve the shape' })).toBeDefined()
    expect(await ui.find({ type: 'Markdown', text: 'The shelf holds **500** books.' })).toBeDefined()
    await ui.unmount()
  }
})

test('where nothing waits on the person, the pane shows no such list', async ($, on) => {
  const w = world(on)
  w.state.read = LOG
  await opened($, w)
  const ui = await mount($, 'desktop')
  expect(await ui.find({ key: 'waiting' })).toBeUndefined()
  await ui.unmount()
})

test('where the working tree holds no Yard Log, the pane says so and names the branch', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ branch: 'feat/new', yardLog: { title: 'feat/new', branch: 'feat/new', closedBy: 'open' }, revision: 0 })
  await opened($, w)
  for (const surface of SURFACES) {
    const ui = await mount($, surface)
    expect((await ui.find({ type: 'Text', text: /No Yard Log/ }))?.text).toContain('feat/new')
    await ui.unmount()
  }
})

test('a block an agent writes, and an answer given on the page, show within 2 seconds', async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  await opened($, w)
  const ui = await mount($, 'desktop')
  w.state.read = readDoc({
    revision: 6,
    since: 4,
    stages: [
      staged(
        'shape',
        'active',
        'Shape it',
        { ...block('readers', 'question', { prompt: 'Which readers does the change serve?', header: 'Readers', options: [] }), answer: { labels: ['Members'], channel: 'page' } },
        block('found', 'note', { markdown: 'Found a gap.' }),
      ),
    ],
    submissions: [{ agent: OWNER, open: [{ key: 's-1', state: 'open', blocks: ['k-readers', 'k-go'] }] }],
  })
  await w.clock.advance(1000)
  expect(await ui.find({ type: 'Markdown', text: 'Found a gap.' })).toBeDefined()
  expect(await ui.find({ key: 'option-k-readers-0' })).toBeUndefined()
  expect(await ui.find({ type: 'Text', text: /Which readers.*answered on page/ })).toBeDefined()
  await ui.unmount()
})
