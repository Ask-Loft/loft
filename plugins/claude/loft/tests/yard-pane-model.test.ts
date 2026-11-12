import { expect, test } from 'claude-code/testing'

import type { PaneView } from '../types'
import { answeredOn, followed, linkOf, waitingIn } from '../hooks/adapter/pane/model'
import { block, readDoc, SESSION, stage, TREE } from './yard-world'

const OWNER = { host: 'claude-code', session: SESSION }

function withBlocks(id: string, status: string, ...blocks: ReturnType<typeof block>[]) {
  return { ...stage(id, status, OWNER), blocks }
}

const FIRST = readDoc({
  revision: 4,
  stages: [
    withBlocks('shape', 'done', block('choice', 'decision', { title: 'Members only', consequence: 'Visitors wait' })),
    withBlocks('build', 'active', block('store', 'task', { title: 'The store', state: 'active' })),
  ],
})

const first = () => followed(undefined, FIRST)!

test('a first read gives the stages in their order, each with its blocks, at its revision', () => {
  const view = first()
  expect(view.branch).toBe('feat/shelf')
  expect(view.tree).toBe(TREE)
  expect(view.revision).toBe(4)
  expect(view.log).toBe('yard-log-1')
  expect(view.stages.map(each => each.id)).toEqual(['shape', 'build'])
  expect(view.stages[1]!.blocks.map(each => each.id)).toEqual(['store'])
})

test('a read since the revision shown replaces each changed block and stage in place', () => {
  const since = readDoc({
    revision: 6,
    since: 4,
    stages: [withBlocks('build', 'active', block('store', 'task', { title: 'The store', state: 'done' }))],
  })
  const view = followed(first(), since)!
  expect(view.revision).toBe(6)
  expect(view.stages.map(each => each.id)).toEqual(['shape', 'build'])
  expect(view.stages[0]!.blocks.map(each => each.id)).toEqual(['choice'])
  expect(view.stages[1]!.blocks.map(each => each.id)).toEqual(['store'])
  expect(view.stages[1]!.blocks[0]!.version.body.state).toBe('done')
})

test('a changed stage keeps the blocks it held before', () => {
  const since = readDoc({ revision: 5, since: 4, stages: [withBlocks('shape', 'dropped')] })
  const view = followed(first(), since)!
  expect(view.stages[0]!.version.body.status).toBe('dropped')
  expect(view.stages[0]!.blocks.map(each => each.id)).toEqual(['choice'])
})

test('a read since that brings a stage or a block the view does not hold asks for a whole read, which places it', () => {
  const view = first()
  expect(followed(view, readDoc({ revision: 5, since: 4, stages: [withBlocks('build', 'active', block('result', 'note', { markdown: 'Built.' }))] }))).toBe(undefined)
  expect(followed(view, readDoc({ revision: 5, since: 4, stages: [withBlocks('opened', 'active')] }))).toBe(undefined)
  // A stage opened after the last unplanned one, before a planned one, shows there once read whole.
  const read = readDoc({ revision: 5, stages: [withBlocks('shape', 'done'), withBlocks('opened', 'active'), withBlocks('build', 'planned')] })
  expect(followed(undefined, read)!.stages.map(each => each.id)).toEqual(['shape', 'opened', 'build'])
})

test('a read of another branch, tree or Yard Log asks for a whole read', () => {
  const view = first()
  expect(followed(view, readDoc({ since: 4, branch: 'feat/other' }))).toBe(undefined)
  expect(followed(view, readDoc({ since: 4, tree: '/work/other' }))).toBe(undefined)
  expect(followed(view, readDoc({ since: 4, yardLog: { key: 'yard-log-2', closedBy: 'open' } }))).toBe(undefined)
})

test('a whole read of another branch starts the view over', () => {
  const other = readDoc({ branch: 'feat/other', revision: 2, stages: [withBlocks('only', 'active')] })
  const view = followed(first(), other)!
  expect(view.branch).toBe('feat/other')
  expect(view.stages.map(each => each.id)).toEqual(['only'])
})

test('a branch with no Yard Log gives a view that says so, with its branch', () => {
  const view = followed(undefined, readDoc({ yardLog: { title: 'feat/new', branch: 'feat/new' }, branch: 'feat/new', stages: [], revision: 0 }))!
  expect(view.log).toBe(null)
  expect(view.branch).toBe('feat/new')
})

const ASKING = readDoc({
  revision: 7,
  stages: [
    withBlocks(
      'shape',
      'active',
      block('readers', 'question', { prompt: 'Which readers?', header: 'Readers', options: [] }),
      block('go', 'approval', { title: 'Go ahead', markdown: 'All of it.' }),
      { ...block('old', 'question', { prompt: 'Old?', header: 'Old', options: [] }), answer: { labels: ['Yes'], channel: 'page' } },
    ),
  ],
  submissions: [{ agent: OWNER, open: [{ key: 's-1', state: 'open', blocks: ['k-go', 'k-readers', 'k-old'] }] }],
})

test('what waits on the person is each unanswered block of an open submission, in the order the submission holds them', () => {
  const view = followed(undefined, ASKING)!
  expect(waitingIn(view).map(each => each.id)).toEqual(['go', 'readers'])
})

test('nothing waits where no submission is open', () => {
  expect(waitingIn(first())).toEqual([])
})

test('a block answered or decided names the channel it was answered on', () => {
  const view = followed(undefined, ASKING)!
  const blocks = view.stages[0]!.blocks
  expect(answeredOn(blocks.find(each => each.id === 'old')!)).toBe('page')
  expect(answeredOn(blocks.find(each => each.id === 'go')!)).toBe(undefined)
  expect(answeredOn({ ...blocks[1]!, outcome: { verdict: 'approved', channel: 'terminal' } })).toBe('terminal')
})

test('the view keeps of each block only what the pane draws', () => {
  const read = readDoc({ stages: [withBlocks('s', 'active', { ...block('n', 'note', { markdown: 'x' }), reached: 3, versionCount: 2 } as never)] })
  const kept = (followed(undefined, read) as PaneView).stages[0]!.blocks[0]!
  expect(Object.keys(kept).sort()).toEqual(['foot', 'id', 'key', 'kind', 'version'])
})

test("the Yard page's loopback address is linked as localhost, which Claude Code Desktop links", () => {
  expect(linkOf('http://127.0.0.1:7411/#tree=%2Fw')).toBe('http://localhost:7411/#tree=%2Fw')
  expect(linkOf('https://yard.example/#x')).toBe('https://yard.example/#x')
})

test('a read since the revision shown that changed nothing gives the view shown, so the pane is not drawn again', () => {
  const view = first()
  expect(followed(view, readDoc({ revision: 4, since: 4, stages: [] }))).toBe(view)
  expect(followed(view, readDoc({ revision: 5, since: 4, stages: [] }))).not.toBe(view)
})

const AGENT = { host: 'claude-code', session: SESSION }
const PERSON = { name: 'Reader', email: 'reader@example.com' }
const act = (at: string, by: Record<string, unknown> = AGENT) => ({ at, by, revision: 1 })

/** A block as a whole read with `--versions` gives it: its current version and every one before. */
function versioned(id: string, kind: string, body: Record<string, unknown>, ats: string[]) {
  const versions = ats.map((at, n) => ({ act: act(at), body, number: n + 1 }))
  return { ...block(id, kind, body), version: versions.at(-1)!, versions, versionCount: versions.length }
}

const HISTORY = readDoc({
  revision: 9,
  stages: [
    {
      ...withBlocks('build', 'active', versioned('store', 'task', { title: 'The store', state: 'done' }, ['2026-10-22T09:00:00Z', '2026-10-22T09:20:00Z'])),
      times: { listed: '2026-10-22T08:50:00Z', started: '2026-10-22T08:55:00Z' },
    },
  ],
  sent: [
    {
      key: 's-1',
      blocks: ['k-q'],
      state: 'answered',
      changes: [
        { what: 'sent', act: act('2026-10-22T09:05:00Z') },
        { what: 'answered', act: act('2026-10-22T09:12:00Z', PERSON) },
      ],
    },
    { key: 's-2', blocks: ['k-r'], state: 'open', changes: [{ what: 'sent', act: act('2026-10-22T09:30:00Z') }] },
  ],
})

test("a whole read keeps each stage's times and owner", () => {
  const stage = followed(undefined, HISTORY)!.stages[0]!
  expect(stage.times).toEqual({ listed: '2026-10-22T08:50:00Z', started: '2026-10-22T08:55:00Z' })
  expect(stage.owner).toEqual(OWNER)
})

test("a whole read gives each block's foot: who wrote its current version, its number, and when it was created and updated", () => {
  const store = followed(undefined, HISTORY)!.stages[0]!.blocks[0]!
  expect(store.foot).toEqual({ by: AGENT, number: 2, created: '2026-10-22T09:00:00Z', updated: '2026-10-22T09:20:00Z' })
})

test('a read since keeps when a changed block was created, and takes its new version', () => {
  const view = followed(undefined, HISTORY)!
  const later = { ...block('store', 'task', { title: 'The store', state: 'dropped' }), version: { act: act('2026-10-22T10:00:00Z'), body: { state: 'dropped' }, number: 3 } }
  const since = readDoc({ revision: 10, since: 9, stages: [{ ...withBlocks('build', 'active', later), times: { started: '2026-10-22T08:55:00Z' } }] })
  expect(followed(view, since)!.stages[0]!.blocks[0]!.foot).toEqual({ by: AGENT, number: 3, created: '2026-10-22T09:00:00Z', updated: '2026-10-22T10:00:00Z' })
})

test('every submission is kept, oldest first, with when it was sent and settled', () => {
  expect(followed(undefined, HISTORY)!.submissions).toEqual([
    { key: 's-1', blocks: ['k-q'], sent: '2026-10-22T09:05:00Z', settled: '2026-10-22T09:12:00Z', isOpen: false },
    { key: 's-2', blocks: ['k-r'], sent: '2026-10-22T09:30:00Z', settled: null, isOpen: true },
  ])
})

/** A node of a `spec-change` report, as `loft` writes it. */
const node = (change: string, kind: string, label: string | null, words: { before?: string; after?: string }) => ({ change, key: `n-${kind}-${label}`, kind, label, name: words.after ?? words.before, words })

/** A `spec-change` block with its report, as a read with `--reports` gives it. */
function specChange(id: string, diff: object[]) {
  return block(id, 'loft', { view: 'spec-change', note: 'The change.', report: { counts: { 'graph.diff': diff.length }, graph: { diff, nodes: [{ key: 'n', kind: 'product' }] }, status: { base: {} } } })
}

const CHANGE = [
  node('added', 'requirement', 'RQ-9', { after: 'The pane shall draw the change.' }),
  node('changed', 'criterion', null, { before: 'No report.', after: 'Its nodes.' }),
  node('removed', 'quality_attribute', 'QA-2', { before: 'Fast.' }),
]

test("a spec-change block keeps its report's nodes, before and after, and nothing else of the report", () => {
  const view = followed(undefined, readDoc({ stages: [withBlocks('build', 'active', specChange('change', CHANGE))] }))!
  const kept = view.stages[0]!.blocks[0]!
  expect(kept.diff).toEqual({
    nodes: [
      { change: 'added', kind: 'requirement', label: 'RQ-9', before: null, after: 'The pane shall draw the change.' },
      { change: 'changed', kind: 'criterion', label: null, before: 'No report.', after: 'Its nodes.' },
      { change: 'removed', kind: 'quality_attribute', label: 'QA-2', before: 'Fast.', after: null },
    ],
    more: 0,
  })
  expect(kept.version.body.report).toBeUndefined()
  expect(kept.version.body.note).toBe('The change.')
})

test("a spec-change report is kept to its first 200 nodes, each side's words cut at 2000 characters", () => {
  const many = Array.from({ length: 250 }, (_, n) => node('changed', 'criterion', `AC-${n}`, { before: 'b'.repeat(5000), after: 'a' }))
  const kept = followed(undefined, readDoc({ stages: [withBlocks('build', 'active', specChange('change', many))] }))!.stages[0]!.blocks[0]!
  expect(kept.diff!.nodes).toHaveLength(200)
  expect(kept.diff!.more).toBe(50)
  expect(kept.diff!.nodes[0]!.before).toBe(`${'b'.repeat(2000)}…`)
})

test('a read since that gives a spec-change block without its report keeps the nodes the pane holds', () => {
  const view = followed(undefined, readDoc({ revision: 4, stages: [withBlocks('build', 'active', specChange('change', CHANGE))] }))!
  const bare = block('change', 'loft', { view: 'spec-change', note: 'The change.' })
  const later = followed(view, readDoc({ revision: 5, since: 4, stages: [withBlocks('build', 'active', bare)] }))!
  expect(later.stages[0]!.blocks[0]!.diff?.nodes).toHaveLength(3)
})

test('a loft block of another view keeps no nodes', () => {
  const findings = block('found', 'loft', { view: 'findings', report: { graph: { diff: CHANGE } } })
  const kept = followed(undefined, readDoc({ stages: [withBlocks('build', 'active', findings)] }))!.stages[0]!.blocks[0]!
  expect(kept.diff).toBeUndefined()
  expect(kept.version.body.report).toBeUndefined()
})
