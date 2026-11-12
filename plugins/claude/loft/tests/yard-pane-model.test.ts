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
  expect(Object.keys(kept).sort()).toEqual(['id', 'key', 'kind', 'version'])
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
