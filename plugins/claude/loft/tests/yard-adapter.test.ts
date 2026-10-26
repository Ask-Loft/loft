import { expect, test } from 'claude-code/testing'

import { contextOf } from '../hooks/adapter/context'
import { activeStage, isAsker } from '../hooks/adapter/log'
import { flagOf, readDoc, SESSION, stage, start, world, yardRuns } from './yard-world'

test('the Claude adapter names its host on protocol two commands, events and heartbeats', async ($, on) => {
  const w = world(on, { protocol: 2 })
  w.state.read = readDoc({ stages: [stage('shape', 'active', { host: 'claude-code', session: SESSION })] })
  w.state.terminal.AskUserQuestion = e => ({ result: { questions: e.questions, answers: { 'Which?': 'A' } }, text: 'ok' })
  await start($)
  await $.tool.call({ tool: 'mcp__loft__yard_read' })
  await $.tool.call({ tool: 'mcp__loft__yard_write', batch: [{ id: 'shape', kind: 'stage', title: 'Shape', status: 'active' }] })
  await $.tool.call({ tool: 'AskUserQuestion', questions: [{ question: 'Which?', header: 'Pick', multiSelect: false, options: [{ label: 'A', description: '' }, { label: 'B', description: '' }] }] })
  await $.tool.call({ tool: 'mcp__loft__yard_withdraw', submission: 'sub-1' })
  await $.tool.call({ tool: 'mcp__loft__yard_close' })
  await $.tool.call({ tool: 'mcp__loft__yard_published', branch: 'feat/shelf' })
  await $.session.end({ reason: 'clear', sessionId: SESSION } as never)
  w.state.session = 'session-2'
  await $.classic.SessionStart({ source: 'clear' })
  await $.turn.start({ text: 'go', turnId: 't1' })
  await w.clock.advance(5000)
  const runs = w.state.runs.filter(run => run.argv.includes('--session'))
  expect(runs.length).toBeGreaterThan(5)
  expect([...new Set(yardRuns(w).map(run => run.split(' ')[0]))].sort()).toEqual(['answer', 'close', 'continue', 'protocol', 'published', 'read', 'serve', 'wait', 'withdraw', 'write'])
  for (const run of runs) expect(flagOf(run.argv, '--host')).toBe('claude-code')
  expect(yardRuns(w)[0]).toBe('protocol --format json')
  expect(w.state.tools).toHaveLength(6)
  expect(w.state.beats[0]).toEqual({ host: 'claude-code', session: SESSION })
  expect(w.state.posts.flatMap(post => post.events).find(event => event.kind === 'turn.start')).toMatchObject({
    host: 'claude-code',
    adapter: {
      format: 'yard-adapter/1', version: '2.2.1',
      capabilities: {
        events: ['turn.start', 'turn.end', 'request', 'tool.start', 'tool.end', 'subagent.start', 'subagent.end', 'compaction', 'plan.start', 'plan.end'],
        nativeQuestions: true, nativePlans: true, taskMirroring: true,
        usage: ['input', 'cacheRead', 'cacheWrite', 'output'],
      },
    },
  })
})

test('protocol one keeps the Claude arguments and events without adapter descriptors', async ($, on) => {
  const w = world(on, { protocol: 1 })
  await start($)
  await $.tool.call({ tool: 'mcp__loft__yard_read' })
  await $.turn.start({ text: 'go', turnId: 't1' })
  await w.clock.settle()
  for (const run of w.state.runs) expect(run.argv).not.toContain('--host')
  for (const event of w.state.posts.flatMap(post => post.events)) expect(event.adapter).toBeUndefined()
  expect(w.state.beats[0]).toEqual({ session: SESSION })
  expect(w.state.tools).toHaveLength(6)
})

test('a foreign host with the same native identifiers is another actor in stages and context', () => {
  const foreign = { host: 'other-agent', session: SESSION }
  const own = { host: 'claude-code', session: SESSION }
  const asker = { host: 'claude-code', sessions: [SESSION] }
  const doc = readDoc({
    stages: [stage('foreign', 'active', foreign), stage('own', 'active', own)],
    submissions: [{ agent: foreign, open: [{ key: 'foreign-sub', blocks: [], state: 'open' }] }],
  })
  expect(activeStage(doc, asker)?.id).toBe('own')
  expect(contextOf(doc, asker)).toContain('`foreign-sub`, 0 blocks, sent by another agent')
  expect(isAsker({ ...foreign, agent: 'a1' }, { ...asker, agent: 'a1' })).toBe(false)
  expect(isAsker({ ...own, agent: 'a1' }, { ...asker, agent: 'a1' })).toBe(true)
  expect(isAsker({ session: SESSION }, asker)).toBe(true)
})
