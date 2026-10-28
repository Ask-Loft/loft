import { expect, test } from 'claude-code/testing'

import { contextOf, LIMIT } from '../hooks/adapter/context'
import { block, readDoc, SESSION, stage, start, world } from './yard-world'

const assignee = (id = 'worker') => ({ host: 'claude-code', session: SESSION, agent: id, agentType: 'builder' })
const work = (id = 'shared', state = 'active', assigned = 'worker') => block(id, 'task', {
  title: 'Original scope', state, detail: 'Keep this detail', type: 'fix', assignee: assignee(assigned),
})
const doc = (state = 'active', assigned = 'worker') => {
  const parent = stage('build', 'active', { host: 'claude-code', session: SESSION })
  parent.blocks = [work('shared', state, assigned)]
  return readDoc({ stages: [parent] })
}

test('a worker updates the explicitly bound task without owning or taking its stage', async ($, on) => {
  const w = world(on)
  w.state.agents.push({ id: 'worker', type: 'builder' })
  w.state.read = doc()
  await start($)
  const result = await $.tool.call({ tool: 'TaskUpdate', taskId: 'native-7', agentId: 'worker',
    status: 'completed', metadata: { yardTask: 'shared' } } as never)
  const writes = w.state.runs.filter(run => run.argv.includes('write'))
  expect(result).toMatchObject({ result: { success: true } })
  expect(writes).toHaveLength(1)
  expect(writes[0]!.argv).toContain('--based-on')
  expect(JSON.parse(writes[0]!.stdin!)).toEqual([{
    id: 'shared', kind: 'task', stage: 'build', title: 'Original scope', state: 'done',
    detail: 'Keep this detail', type: 'fix', assignee: assignee(),
  }])
})

test('invalid explicit bindings never create replacement native tasks', async ($, on) => {
  const w = world(on)
  w.state.terminal.TaskCreate = () => ({ result: { task: { id: '3' } }, text: 'ok' })
  w.state.read = doc()
  await start($)
  for (const yardTask of ['missing', '', '../wrong', 17]) {
    await $.tool.call({ tool: 'TaskCreate', subject: 'Duplicate', description: '', metadata: { yardTask } } as never)
  }
  expect(w.state.runs.filter(run => run.argv.includes('write'))).toHaveLength(0)
})

test('a creator mirror preserves a task delegated to another executor', async ($, on) => {
  const w = world(on)
  const held = doc()
  const parent = (held.stages as ReturnType<typeof stage>[])[0]!
  parent.blocks[0]!.id = 'task-session1-3'
  w.state.read = held
  await start($)
  await $.tool.call({ tool: 'TaskUpdate', taskId: '3', status: 'pending' } as never)
  expect(w.state.runs.filter(run => run.argv.includes('write'))).toHaveLength(0)
})

test('unchanged and terminal delegated updates create no versions', async ($, on) => {
  const w = world(on)
  w.state.agents.push({ id: 'worker', type: 'builder' })
  await start($)
  for (const [held, status] of [['active', 'in_progress'], ['done', 'in_progress'], ['dropped', 'completed']]) {
    w.state.read = doc(held)
    await $.tool.call({ tool: 'TaskUpdate', taskId: '7', agentId: 'worker', status, metadata: { yardTask: 'shared' } } as never)
  }
  expect(w.state.runs.filter(run => run.argv.includes('write'))).toHaveLength(0)
})

test('an identified worker end reopens its unfinished assignment and keeps the assignee', async ($, on) => {
  const w = world(on)
  w.state.agents.push({ id: 'worker', type: 'builder' })
  w.state.read = doc()
  await start($)
  await $.turn.complete({ agentId: 'worker', turnId: 'worker-turn', answer: '', durationMs: 1, reason: 'answer', isAborted: false } as never)
  const writes = w.state.runs.filter(run => run.argv.includes('write'))
  expect(writes).toHaveLength(1)
  expect(JSON.parse(writes[0]!.stdin!)[0]).toMatchObject({ id: 'shared', stage: 'build', state: 'open', assignee: assignee() })
})

test('worker end leaves completed and reassigned tasks untouched', async ($, on) => {
  const w = world(on)
  w.state.agents.push({ id: 'worker', type: 'builder' })
  await start($)
  for (const held of [doc('done'), doc('active', 'other')]) {
    w.state.read = held
    await $.turn.complete({ agentId: 'worker', turnId: 'worker-turn', answer: '', durationMs: 1, reason: 'answer', isAborted: false } as never)
  }
  expect(w.state.runs.filter(run => run.argv.includes('write'))).toHaveLength(0)
})

test('assignment context names existing tasks and stages without an owned stage', () => {
  const text = contextOf(doc(), { sessions: [SESSION], agent: 'worker' })
  expect(text).toContain('shared')
  expect(text).toContain('build')
  expect(text).toContain('assigned')
  expect(text).toContain('no active stage')
})

test('assignment context bounds its list and directs the worker to read the rest', () => {
  const held = doc()
  const parent = (held.stages as ReturnType<typeof stage>[])[0]!
  parent.blocks = Array.from({ length: 150 }, (_, n) => work(`assigned-${n}-${'x'.repeat(40)}`))
  const text = contextOf(held, { sessions: [SESSION], agent: 'worker' })
  expect(text.length).toBeLessThanOrEqual(LIMIT)
  expect(text).toContain('more')
  expect(text).toContain('yard_read')
})

test('native subagent start supplies its assigned task context without taking the stage', async ($, on) => {
  const w = world(on)
  w.state.read = doc('open')
  await start($)
  const result = await $.classic.SubagentStart({ agent_id: 'worker', agent_type: 'builder' } as never)
  expect(result.additionalContext?.join('\n')).toContain('shared')
  expect(w.state.runs.filter(run => run.argv.includes('write'))).toHaveLength(0)
})

test('an ended worker cannot mirror late delegated progress until an identified new start', async ($, on) => {
  const w = world(on)
  w.state.agents.push({ id: 'worker', type: 'builder' })
  w.state.read = doc()
  await start($)
  await $.turn.complete({ agentId: 'worker', turnId: 'worker-turn', answer: '', durationMs: 1, reason: 'answer', isAborted: false } as never)
  w.state.read = doc('open')
  await $.tool.call({ tool: 'TaskUpdate', taskId: '7', agentId: 'worker', status: 'in_progress', metadata: { yardTask: 'shared' } } as never)
  expect(w.state.runs.filter(run => run.argv.includes('write'))).toHaveLength(1)
  await $.classic.SubagentStart({ agent_id: 'worker', agent_type: 'builder' } as never)
  await $.tool.call({ tool: 'TaskUpdate', taskId: '7', agentId: 'worker', status: 'in_progress', metadata: { yardTask: 'shared' } } as never)
  expect(w.state.runs.filter(run => run.argv.includes('write'))).toHaveLength(2)
})

test('stale retries reread the task and stop when another worker received its assignment', async ($, on) => {
  const w = world(on)
  w.state.agents.push({ id: 'worker', type: 'builder' })
  w.state.read = doc()
  let writes = 0
  w.state.writing = () => {
    writes++
    w.state.read = { ...doc('active', 'other'), revision: 5 }
    return { exitCode: 1, doc: { outcome: 'refused', errors: [{ kind: 'stale' }] } }
  }
  await start($)
  const result = await $.tool.call({ tool: 'TaskUpdate', taskId: '7', agentId: 'worker', status: 'completed', metadata: { yardTask: 'shared' } } as never)
  expect(result).toMatchObject({ result: { success: true } })
  expect(writes).toBe(1)
})

test('a stale retry preserves unrelated fields from the latest task body', async ($, on) => {
  const w = world(on)
  w.state.agents.push({ id: 'worker', type: 'builder' })
  w.state.read = doc()
  let writes = 0
  w.state.writing = () => {
    if (++writes > 1) return undefined
    const latest = doc()
    const parent = (latest.stages as ReturnType<typeof stage>[])[0]!
    parent.blocks[0]!.version.body.detail = 'Concurrent detail'
    w.state.read = { ...latest, revision: 5 }
    return { exitCode: 1, doc: { outcome: 'refused', errors: [{ kind: 'stale' }] } }
  }
  await start($)
  await $.tool.call({ tool: 'TaskUpdate', taskId: '7', agentId: 'worker', status: 'completed', metadata: { yardTask: 'shared' } } as never)
  const saved = w.state.runs.filter(run => run.argv.includes('write'))
  expect(saved).toHaveLength(2)
  expect(saved[1]!.argv).toContain('5')
  expect(JSON.parse(saved[1]!.stdin!)[0]).toMatchObject({ detail: 'Concurrent detail', assignee: assignee(), state: 'done' })
})

test('a bound native create leaves the existing task identity and scope intact', async ($, on) => {
  const w = world(on)
  w.state.terminal.TaskCreate = () => ({ result: { task: { id: '3' } }, text: 'ok' })
  w.state.read = doc('open')
  await start($)
  const result = await $.tool.call({ tool: 'TaskCreate', subject: 'Local checklist', description: '', metadata: { yardTask: 'shared' } } as never)
  expect(result).toMatchObject({ result: { task: { id: '3' } } })
  expect(w.state.runs.filter(run => run.argv.includes('write'))).toHaveLength(0)
})

test('an actual interruption returns unfinished execution to open without changing the native answer', async ($, on) => {
  const w = world(on)
  w.state.read = doc()
  await start($)
  const result = await $.turn.complete({ agentId: 'worker', turnId: 't1', answer: '', durationMs: 1, reason: 'aborted', isAborted: true } as never)
  expect(result).toMatchObject({ text: '' })
  const writes = w.state.runs.filter(run => run.argv.includes('write'))
  expect(writes).toHaveLength(1)
  expect(JSON.parse(writes[0]!.stdin!)[0]).toMatchObject({ id: 'shared', state: 'open', assignee: assignee() })
})

test('long page addresses leave room for assignment counts and a read instruction', () => {
  const held = doc()
  held.branch = 'b'.repeat(2500)
  held.page = { url: `http://127.0.0.1:7411/${'x'.repeat(2500)}` }
  const parent = (held.stages as ReturnType<typeof stage>[])[0]!
  parent.blocks = Array.from({ length: 150 }, (_, n) => work(`assigned-${n}-${'x'.repeat(40)}`))
  const text = contextOf(held, { sessions: [SESSION], agent: 'worker' })
  expect(text.length).toBeLessThanOrEqual(LIMIT)
  expect(text).toContain('more')
  expect(text).toContain('yard_read')
})
