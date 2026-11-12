import { expect, test } from 'claude-code/testing'

import { block, readDoc, stage, start, world, yardRuns, type Engine, type World } from './yard-world'

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
