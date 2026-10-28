import { expect, test } from 'claude-code/testing'

import { block, briefingOf, readDoc, SESSION, stage, start, world, yardRuns, type World } from './yard-world'

const assignee = (id = 'worker') => ({ host: 'claude-code', session: SESSION, agent: id, agentType: 'builder' })
const work = (id = 'shared', state = 'active', assigned = 'worker') => block(id, 'task', {
  title: 'Original scope', state, detail: 'Keep this detail', type: 'fix', assignee: assignee(assigned),
})
const doc = (state = 'active', assigned = 'worker') => {
  const parent = stage('build', 'active', { host: 'claude-code', session: SESSION })
  parent.blocks = [work('shared', state, assigned)]
  return readDoc({ stages: [parent] })
}
const WORKER = `--session ${SESSION} --agent worker --agent-type builder --host claude-code --port 7411 --format json`
const taskRuns = (w: World) => yardRuns(w).filter(run => run.startsWith('task '))
const writes = (w: World) => w.state.runs.filter(run => run.argv.includes('write'))

test('a worker updates the explicitly bound task without owning or taking its stage', async ($, on) => {
  const w = world(on, { protocol: 2 })
  w.state.agents.push({ id: 'worker', type: 'builder' })
  w.state.read = doc()
  await start($)
  const before = yardRuns(w).length
  const result = await $.tool.call({ tool: 'TaskUpdate', taskId: 'native-7', agentId: 'worker',
    status: 'completed', metadata: { yardTask: 'shared' } } as never)
  expect(result).toMatchObject({ result: { success: true } })
  expect(yardRuns(w).slice(before)).toEqual([`task shared --mirror --state done ${WORKER}`])
  expect(writes(w)).toHaveLength(0)
})

test('invalid explicit bindings never create replacement native tasks', async ($, on) => {
  const w = world(on)
  w.state.terminal.TaskCreate = () => ({ result: { task: { id: '3' } }, text: 'ok' })
  w.state.read = doc()
  await start($)
  for (const yardTask of ['missing', '', '../wrong', 17]) {
    await $.tool.call({ tool: 'TaskCreate', subject: 'Duplicate', description: '', metadata: { yardTask } } as never)
  }
  expect(writes(w)).toHaveLength(0)
  expect(taskRuns(w)).toHaveLength(0)
})

test("each native update is mirrored once with loft yard task --mirror, which leaves the delegation rules to Yard Log, with no read first and no retry", async ($, on) => {
  const w = world(on, { protocol: 2 })
  w.state.agents.push({ id: 'worker', type: 'builder' })
  w.state.read = doc()
  w.state.answers.task = { exitCode: 0, doc: { outcome: 'unchanged', errors: [], task: 'shared', state: 'done', revision: 4 } }
  await start($)
  const before = w.state.runs.length
  for (const [agentId, status] of [[undefined, 'pending'], ['worker', 'in_progress'], ['worker', 'completed']] as const) {
    await $.tool.call({ tool: 'TaskUpdate', taskId: '3', status, ...(agentId ? { agentId, metadata: { yardTask: 'shared' } } : {}) } as never)
  }
  const runs = w.state.runs.slice(before).map(run => run.argv.slice(run.argv.indexOf('yard') + 1).join(' '))
  expect(runs).toEqual([
    `task task-session1-3 --mirror --state open --session ${SESSION} --host claude-code --port 7411 --format json`,
    `task shared --mirror --state active ${WORKER}`,
    `task shared --mirror --state done ${WORKER}`,
  ])
})

test('an identified worker end returns its active tasks to open in one loft yard task --release', async ($, on) => {
  const w = world(on, { protocol: 2 })
  w.state.agents.push({ id: 'worker', type: 'builder' })
  w.state.read = doc()
  await start($)
  const before = yardRuns(w).length
  await $.turn.complete({ agentId: 'worker', turnId: 'worker-turn', answer: '', durationMs: 1, reason: 'answer', isAborted: false } as never)
  expect(yardRuns(w).slice(before)).toEqual([`task --release ${WORKER}`])
  expect(writes(w)).toHaveLength(0)
})

test("the main agent's turn end and its session's end each release the main agent's tasks", async ($, on) => {
  const w = world(on, { protocol: 2 })
  await start($)
  await $.turn.complete({ turnId: 't1', answer: '', durationMs: 1, reason: 'answer', isAborted: false } as never)
  await $.session.end({ reason: 'other', sessionId: SESSION } as never)
  expect(taskRuns(w)).toEqual([
    `task --release --session ${SESSION} --host claude-code --port 7411 --format json`,
    `task --release --session ${SESSION} --host claude-code --port 7411 --format json`,
  ])
})

test('native subagent start gives the worker its briefing, marked read in its own name, without taking the stage', async ($, on) => {
  const w = world(on)
  w.state.read = doc('open')
  await start($)
  const before = w.state.runs.length
  const result = await $.classic.SubagentStart({ agent_id: 'worker', agent_type: 'builder' } as never)
  const reads = w.state.runs.slice(before).filter(run => run.argv.includes('read'))
  expect(reads).toHaveLength(1)
  expect(reads[0]!.argv).toEqual(expect.arrayContaining(['--context', '--mark-read', '--agent', 'worker', '--agent-type', 'builder']))
  const briefing = briefingOf(w.state.read, reads[0]!.argv)
  expect(result.additionalContext).toEqual([`${String(briefing.text)}\nRead Yard Log with \`yard_read\` before writing.`])
  expect(String(briefing.text)).toContain('`shared`, open, in stage `build`')
  expect(writes(w)).toHaveLength(0)
})

test('an ended worker cannot mirror late delegated progress until an identified new start', async ($, on) => {
  const w = world(on)
  w.state.agents.push({ id: 'worker', type: 'builder' })
  w.state.read = doc()
  await start($)
  await $.turn.complete({ agentId: 'worker', turnId: 'worker-turn', answer: '', durationMs: 1, reason: 'answer', isAborted: false } as never)
  w.state.read = doc('open')
  await $.tool.call({ tool: 'TaskUpdate', taskId: '7', agentId: 'worker', status: 'in_progress', metadata: { yardTask: 'shared' } } as never)
  expect(taskRuns(w).filter(run => run.startsWith('task shared'))).toHaveLength(0)
  await $.classic.SubagentStart({ agent_id: 'worker', agent_type: 'builder' } as never)
  await $.tool.call({ tool: 'TaskUpdate', taskId: '7', agentId: 'worker', status: 'in_progress', metadata: { yardTask: 'shared' } } as never)
  expect(taskRuns(w).filter(run => run.startsWith('task shared'))).toHaveLength(1)
})

test('a bound native create leaves the existing task identity and scope intact', async ($, on) => {
  const w = world(on)
  w.state.terminal.TaskCreate = () => ({ result: { task: { id: '3' } }, text: 'ok' })
  w.state.read = doc('open')
  await start($)
  const result = await $.tool.call({ tool: 'TaskCreate', subject: 'Local checklist', description: '', metadata: { yardTask: 'shared' } } as never)
  expect(result).toMatchObject({ result: { task: { id: '3' } } })
  expect(writes(w)).toHaveLength(0)
})

test('an actual interruption returns unfinished execution to open without changing the native answer', async ($, on) => {
  const w = world(on)
  w.state.read = doc()
  await start($)
  const result = await $.turn.complete({ agentId: 'worker', turnId: 't1', answer: '', durationMs: 1, reason: 'aborted', isAborted: true } as never)
  expect(result).toMatchObject({ text: '' })
  expect(taskRuns(w)).toHaveLength(1)
  expect(taskRuns(w)[0]).toMatch(/^task --release --session session-1 --agent worker /)
})
