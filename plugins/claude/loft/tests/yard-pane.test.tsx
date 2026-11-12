import { expect, test, type Mounted } from 'claude-code/testing'

import { block, deferred, PAGE, readDoc, SESSION, stage, start, world, yardRuns, type Engine, type World } from './yard-world'

const OWNER = { host: 'claude-code', session: SESSION }

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
  const w = world(on)
  w.state.read = readDoc({ stages: [{ ...stage('build', 'active', OWNER), blocks: [block('store', 'task', { title: 'The store', state: 'open' })] }] })
  await opened($, w)
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
const FIELDED = ['terminal', 'desktop'] as const

const PANE = {
  plugin: 'loft',
  component: 'Pane',
  requestId: 'yard',
  props: { title: 'Yard', isFocused: false, bodyColumns: 80, placement: 'dock', scroll: { offset: 0, bodyRows: 40 }, view: {} },
} as const

/** Mounts the pane on `surface`, as Claude Code draws it there. */
const mount = ($: Engine, surface: (typeof SURFACES)[number]) => $.ui.mount({ ...PANE, surface } as never)

/** Mounts the pane on a surface that draws a text field, typing into it as the person would. */
const mountFielded = ($: Engine, surface: (typeof FIELDED)[number]) => $.ui.mount({ ...PANE, surface } as never) as unknown as Promise<Mounted<'desktop'>>

function staged(id: string, status: string, goal: string, ...blocks: ReturnType<typeof block>[]) {
  const held = stage(id, status, OWNER)
  return { ...held, version: { body: { ...held.version.body, goal } }, blocks }
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

test('what waits on the person shows its questions with their options, and its approvals with their Markdown', async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  await opened($, w)
  for (const surface of SURFACES) {
    const ui = await mount($, surface)
    expect(await ui.find({ key: 'waiting' })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: 'Readers' })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: 'Which readers does the change serve?' })).toBeDefined()
    expect(await ui.find({ type: 'Markdown', text: 'Cards are **new**.' })).toBeDefined()
    expect((await ui.find({ key: 'option-k-readers-0' }))?.text).toBe('Members')
    expect((await ui.find({ key: 'pick-k-readers-0' }))?.text).toMatch(/^A.*Members.*Recommended/)
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

test('a write that changes what the pane shows, and an answer given on the page, show within 2 seconds', async ($, on) => {
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
        block('found', 'task', { title: 'Close the gap', state: 'active' }),
      ),
    ],
    submissions: [{ agent: OWNER, open: [{ key: 's-1', state: 'open', blocks: ['k-readers', 'k-go'] }] }],
  })
  await w.clock.advance(1000)
  expect((await ui.find({ key: 'task-k-found' }))?.text).toContain('Close the gap')
  expect(await ui.find({ key: 'option-k-readers-0' })).toBeUndefined()
  await ui.unmount()
})

const MULTIPLE = readDoc({
  stages: [
    staged(
      'shape',
      'active',
      'Shape it',
      block('shelves', 'question', {
        prompt: 'Which shelves?',
        header: 'Shelves',
        multiple: true,
        options: [
          { label: 'North', recommended: false },
          { label: 'South', recommended: false },
          { label: 'East', recommended: false },
        ],
      }),
    ),
  ],
  submissions: [{ agent: OWNER, open: [{ key: 's-2', state: 'open', blocks: ['k-shelves'] }] }],
})

/** What each `loft yard answer` was given, with the words after `yard` but the port and format. */
const answered = (w: World) =>
  w.state.runs
    .filter(run => run.argv.includes('answer'))
    .map(run => ({ args: run.argv.slice(run.argv.indexOf('yard') + 1).join(' ').replace(' --port 7411 --format json', ''), given: JSON.parse(run.stdin ?? '[]') }))

test('choosing an option of a question that takes one records it with loft yard answer, for the tree the pane shows', async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  await opened($, w)
  for (const surface of SURFACES) {
    const before = answered(w).length
    const ui = await mount($, surface)
    await ui.press({ key: 'option-k-readers-1' })
    expect(answered(w).slice(before)).toEqual([{ args: 'answer --input - --session session-1 --tree /work/shelf', given: [{ block: 'k-readers', labels: ['Visitors'] }] }])
    await ui.unmount()
  }
})

test('a question that takes more than one option records the options ticked once sent', async ($, on) => {
  const w = world(on)
  w.state.read = MULTIPLE
  await opened($, w)
  const ui = await mount($, 'mobile')
  await ui.press({ key: 'option-k-shelves-0' })
  await ui.press({ key: 'option-k-shelves-2' })
  await ui.press({ key: 'option-k-shelves-1' })
  await ui.press({ key: 'option-k-shelves-1' })
  expect((await ui.find({ key: 'option-k-shelves-0' }))?.text).toBe('☑ North')
  expect((await ui.find({ key: 'option-k-shelves-1' }))?.text).toBe('☐ South')
  expect(answered(w)).toEqual([])
  await ui.press({ key: 'send-k-shelves' })
  expect(answered(w).map(each => each.given)).toEqual([[{ block: 'k-shelves', labels: ['North', 'East'] }]])
  await ui.unmount()
})

test("words typed in a question's field and sent are recorded as the person's own words", async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  await opened($, w)
  for (const surface of FIELDED) {
    const before = answered(w).length
    const ui = await mountFielded($, surface)
    await ui.input({ key: 'words-k-readers', text: 'Members and their ', kind: 'change' })
    await ui.input({ key: 'words-k-readers', text: 'Members and their guests' })
    expect(answered(w).slice(before).map(each => each.given)).toEqual([[{ block: 'k-readers', words: 'Members and their guests' }]])
    await ui.unmount()
  }
})

test('Approve records the block approved, and Reject rejected with the words typed as its reason, or with none', async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  await opened($, w)
  for (const surface of FIELDED) {
    const before = answered(w).length
    const ui = await mountFielded($, surface)
    await ui.press({ key: 'approve-k-go' })
    await ui.input({ key: 'reason-k-go', text: 'Too many books', kind: 'change' })
    await ui.press({ key: 'reject-k-go' })
    await ui.press({ key: 'reject-k-go' })
    await ui.input({ key: 'reason-k-go', text: 'Say why' })
    expect(answered(w).slice(before).map(each => each.given)).toEqual([
      [{ block: 'k-go', verdict: 'approved' }],
      [{ block: 'k-go', verdict: 'rejected', reason: 'Too many books' }],
      [{ block: 'k-go', verdict: 'rejected' }],
      [{ block: 'k-go', verdict: 'rejected', reason: 'Say why' }],
    ])
    await ui.unmount()
  }
})

test('where the surface has no text field, a question is answered by its options and Reject records no reason', async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  await opened($, w)
  const ui = await mount($, 'mobile')
  expect(await ui.findAll({ type: 'Input' })).toEqual([])
  await ui.press({ key: 'reject-k-go' })
  expect(answered(w).map(each => each.given)).toEqual([[{ block: 'k-go', verdict: 'rejected' }]])
  await ui.unmount()
})

test('once recorded, the block no longer shows as waiting on the person', async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  await opened($, w)
  const ui = await mount($, 'desktop')
  w.state.read = readDoc({
    revision: 5,
    since: 4,
    stages: [
      staged('shape', 'active', 'Shape it', { ...block('go', 'approval', { title: 'Approve the shape', markdown: '' }), outcome: { verdict: 'approved', channel: 'terminal' } }),
    ],
    submissions: [{ agent: OWNER, open: [{ key: 's-1', state: 'open', blocks: ['k-readers', 'k-go'] }] }],
  })
  await ui.press({ key: 'approve-k-go' })
  expect(await ui.find({ key: 'approve-k-go' })).toBeUndefined()
  expect(await ui.find({ key: 'option-k-readers-0' })).toBeDefined()
  await ui.unmount()
})

test('a block answered elsewhere first keeps that answer, and the pane names where it was answered', async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  w.state.answers.answer = {
    doc: { outcome: 'answered elsewhere', errors: [{ message: 'block `readers` was answered first on the page; its answer is kept' }], revision: 5 },
    exitCode: 1,
  }
  await opened($, w)
  const ui = await mount($, 'desktop')
  w.state.read = readDoc({
    revision: 5,
    since: 4,
    stages: [staged('shape', 'active', 'Shape it', { ...block('readers', 'question', { prompt: 'Which readers does the change serve?', options: [] }), answer: { labels: ['Members'], channel: 'page' } })],
    submissions: [{ agent: OWNER, open: [{ key: 's-1', state: 'open', blocks: ['k-readers', 'k-go'] }] }],
  })
  await ui.press({ key: 'option-k-readers-1' })
  expect(await ui.find({ type: 'Text', text: /answered first on the page/ })).toBeDefined()
  expect(await ui.find({ key: 'option-k-readers-0' })).toBeUndefined()
  expect(answered(w)).toHaveLength(1)
  await ui.unmount()
})

test("Claude Code's dialog for the same submission closes once the pane has answered the last of it", async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  let asked = 0
  w.state.terminal.AskUserQuestion = () => {
    asked++
    return new Promise(() => {})
  }
  await start($)
  const waited = $.tool.call({ tool: 'mcp__loft__yard_wait', submission: 's-1' })
  await w.clock.settle()
  expect(asked).toBe(1)
  await $.command.run({ command: 'yard-pane', args: '' } as never)
  const ui = await mount($, 'desktop')
  // loft yard wait ends once the pane's answer leaves nothing of the submission waiting.
  w.state.onAnswer = () => w.state.page.resolve({ outcome: 'answered', errors: [], blocks: [], takenUp: true, waited: 1 })
  await ui.press({ key: 'approve-k-go' })
  const ran = await waited
  await w.clock.settle()
  expect(JSON.parse(String(ran.result)).outcome).toBe('answered')
  expect(asked).toBe(1)
  expect(yardRuns(w).filter(run => run.startsWith('answer'))).toHaveLength(1)
  await ui.unmount()
})

test('an answer loft does not record keeps the words typed for another try, and the pane says why', async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  w.state.answers.answer = { doc: { outcome: 'error', errors: [{ message: 'Yard Log is locked' }] }, exitCode: 1 }
  await opened($, w)
  const ui = await mountFielded($, 'desktop')
  await ui.input({ key: 'reason-k-go', text: 'Too many books', kind: 'change' })
  await ui.press({ key: 'reject-k-go' })
  expect(await ui.find({ type: 'Text', text: /Yard Log is locked/ })).toBeDefined()
  expect((await ui.find({ key: 'reason-k-go' }))?.text).toBe('Too many books')
  await ui.unmount()
})

test("choosing an option records the words typed in the question's field with it", async ($, on) => {
  const w = world(on)
  w.state.read = ASKING
  await opened($, w)
  const ui = await mountFielded($, 'terminal')
  await ui.input({ key: 'words-k-readers', text: 'and their guests', kind: 'change' })
  await ui.press({ key: 'option-k-readers-0' })
  expect(answered(w).map(each => each.given)).toEqual([[{ block: 'k-readers', labels: ['Members'], words: 'and their guests' }]])
  await ui.unmount()
})

test('running the command while a read is under way reads Yard Log whole once that read ends', async ($, on) => {
  const w = await opened($, world(on))
  const held = deferred<void>()
  w.state.reading = held.promise
  await w.clock.advance(1000)
  await $.command.run({ command: 'yard-pane', args: '' } as never)
  held.resolve()
  await w.clock.settle()
  expect(reads(w).slice(-2)).toEqual(['read --session session-1 --since 4', 'read --session session-1'])
})

test('a read since that brings a block the pane does not show is followed by a whole read', async ($, on) => {
  const w = await opened($, world(on))
  w.state.read = readDoc({ revision: 5, since: 4, stages: [{ ...stage('build', 'active', OWNER), blocks: [block('found', 'note', { markdown: 'Found.' })] }] })
  await w.clock.advance(1000)
  expect(reads(w).slice(-2)).toEqual(['read --session session-1 --since 4', 'read --session session-1'])
})
