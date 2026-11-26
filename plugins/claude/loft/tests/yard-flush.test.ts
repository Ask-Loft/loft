import { expect, test } from 'claude-code/testing'

import { deferred, readDoc, SESSION, stage, start, world, yardRuns, type Engine, type World } from './yard-world'

const FLUSH = 'flush --format json'
const flushes = (w: World) => yardRuns(w).filter(run => run === FLUSH).length
const questions = [{ question: 'Proceed?', header: 'Next', multiSelect: false, options: [{ label: 'Yes', description: '' }, { label: 'No', description: '' }] }]
const note = { batch: [{ id: 'n1', kind: 'note', stage: 'shape', markdown: 'x' }] }

/** A world whose `loft` speaks protocol 5, where Yard Log has the main agent's active stage. */
function flushing(on: Parameters<typeof world>[0]) {
  const w = world(on, { protocol: 5 })
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  return w
}

/** The flush answers as `loft yard flush` does with `outcome`, counting `remaining` entries. */
function answering(w: World, outcome: string, remaining = 0) {
  w.state.answers.flush = { doc: { schema: 'loft-yard-flush/1', outcome, sent: 0, remaining, later: 0, skipped: 0, dead: 0, errors: [] }, exitCode: outcome === 'failed' ? 1 : 0 }
}

test("the session's first beat runs loft yard flush", async ($, on) => {
  const w = flushing(on)
  await start($)
  await w.clock.settle()
  expect(flushes(w)).toBe(1)
})

test('after a write through Yard\'s tools the mod runs loft yard flush, and the call does not wait for it', async ($, on) => {
  const w = flushing(on)
  answering(w, 'empty')
  await start($)
  await w.clock.settle()
  const held = deferred<void>()
  w.state.flushing = held.promise
  const result = await $.tool.call({ tool: 'mcp__loft__yard_write', ...note } as never)
  expect(result.result).toBeDefined()
  expect(yardRuns(w).at(-1)).toBe(FLUSH)
  expect(flushes(w)).toBe(2)
  held.resolve()
})

test('after a write mirrored from a task, a question or a plan the mod runs loft yard flush, and the native call does not wait for it', async ($, on) => {
  const w = flushing(on)
  answering(w, 'empty')
  w.state.terminal.TaskCreate = () => ({ result: { task: { id: '3' } }, text: 'created' })
  w.state.terminal.AskUserQuestion = () => ({ result: { questions, answers: { 'Proceed?': 'Yes' } } })
  w.state.terminal.ExitPlanMode = () => ({ result: {} })
  await start($)
  await w.clock.settle()
  for (const call of [
    { tool: 'TaskCreate', subject: 'Task', description: '' },
    { tool: 'AskUserQuestion', questions },
    { tool: 'ExitPlanMode', plan: '# Plan' },
  ]) {
    const held = deferred<void>()
    w.state.flushing = held.promise
    const before = flushes(w)
    const ran = await $.tool.call(call as never)
    await w.clock.settle()
    expect(ran.deny).toBeUndefined()
    expect(flushes(w)).toBe(before + 1)
    held.resolve()
    await w.clock.settle()
  }
})

/** Whether a beat after a first flush that ended `outcome`, counting `remaining`, runs another. */
async function beatsAgain($: Engine, w: World, outcome: string, remaining = 0) {
  answering(w, outcome, remaining)
  await start($)
  await w.clock.settle()
  const before = flushes(w)
  await w.clock.advance(5000)
  await w.clock.settle()
  return flushes(w) > before
}

test('a beat runs loft yard flush where the last one ended failed', async ($, on) => {
  expect(await beatsAgain($, flushing(on), 'failed')).toBe(true)
})

test('a beat runs loft yard flush where the last one ended busy', async ($, on) => {
  expect(await beatsAgain($, flushing(on), 'busy')).toBe(true)
})

test('a beat runs loft yard flush where the last one ended paused', async ($, on) => {
  expect(await beatsAgain($, flushing(on), 'paused')).toBe(true)
})

test('a beat runs loft yard flush where the last one counted entries remaining', async ($, on) => {
  expect(await beatsAgain($, flushing(on), 'sent', 2)).toBe(true)
})

test('a beat runs no flush where the last one ended sent with no entry remaining', async ($, on) => {
  expect(await beatsAgain($, flushing(on), 'sent')).toBe(false)
})

test('a beat runs no flush where the last one ended empty', async ($, on) => {
  expect(await beatsAgain($, flushing(on), 'empty')).toBe(false)
})

test('once a flush has ended off, the mod runs no flush for the rest of the session', async ($, on) => {
  const w = flushing(on)
  answering(w, 'off')
  await start($)
  await w.clock.settle()
  await $.tool.call({ tool: 'mcp__loft__yard_write', ...note } as never)
  await w.clock.advance(15000)
  await w.clock.settle()
  expect(flushes(w)).toBe(1)
})

test('the mod starts no flush while one it started runs, and one write made meanwhile is followed by one more flush', async ($, on) => {
  const w = flushing(on)
  answering(w, 'empty')
  const held = deferred<void>()
  w.state.flushing = held.promise
  await start($)
  await w.clock.settle()
  expect(flushes(w)).toBe(1)
  await $.tool.call({ tool: 'mcp__loft__yard_write', ...note } as never)
  await $.tool.call({ tool: 'mcp__loft__yard_write', ...note } as never)
  await w.clock.advance(5000)
  await w.clock.settle()
  expect(flushes(w)).toBe(1)
  w.state.flushing = Promise.resolve()
  held.resolve()
  await w.clock.settle()
  expect(flushes(w)).toBe(2)
})

test('the mod has Claude Code stop a flush it started that still runs 6 minutes later', async ($, on) => {
  const w = flushing(on)
  await start($)
  await w.clock.settle()
  const run = w.state.runs.find(each => each.argv.includes('flush'))!
  expect(run.timeoutMs).toBe(6 * 60_000)
})

test('the mod runs loft yard flush only where the loft it runs speaks protocol 5 or a later one', async ($, on) => {
  const w = world(on, { protocol: 4 })
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  await start($)
  await $.tool.call({ tool: 'mcp__loft__yard_write', ...note } as never)
  await w.clock.advance(15000)
  await w.clock.settle()
  expect(flushes(w)).toBe(0)
})

test('a flush the mod starts runs as every loft command does, adding only the quiet variables to the session\'s environment', async ($, on) => {
  const w = flushing(on)
  await start($)
  await w.clock.settle()
  const run = w.state.runs.find(each => each.argv.includes('flush'))!
  expect(run.env).toEqual({ LOFT_TELEMETRY: 'off', LOFT_UPDATE_CHECK: 'off' })
  expect(run.argv).not.toContain('--port')
})
