import { expect, test } from 'claude-code/testing'
import { deferred, readDoc, SESSION, stage, start, world } from './yard-world'

const questions = [{ question: 'Proceed?', header: 'Next', multiSelect: false, options: [{ label: 'Yes', description: '' }, { label: 'No', description: '' }] }]
const created = { result: { task: { id: '3' } }, text: 'native result' }
const writes = (w: ReturnType<typeof world>) => w.state.runs.filter(r => r.argv.includes('write')).map(r => JSON.parse(r.stdin ?? '[]') as Record<string, unknown>[])

test('task capture cannot be overtaken by later plans questions or Yard writes', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  w.state.terminal.ExitPlanMode = () => ({ result: {} })
  w.state.terminal.AskUserQuestion = () => ({ result: { questions, answers: { 'Proceed?': 'Yes' } } })
  await start($)
  for (const tool of ['ExitPlanMode', 'AskUserQuestion', 'mcp__loft__yard_write']) {
    const native = deferred<typeof created>()
    w.state.terminal.TaskCreate = () => native.promise
    const before = writes(w).length
    const task = $.tool.call({ tool: 'TaskCreate', subject: 'Task', description: '' } as never)
    await w.clock.settle()
    const later = $.tool.call({ tool, plan: '# Plan', questions, batch: [{ id: 'note', kind: 'note', stage: 'shape', markdown: 'Later' }] } as never)
    await w.clock.settle()
    const overtook = writes(w).length > before
    native.resolve(created)
    await Promise.all([task, later])
    await w.clock.settle()
    expect(overtook).toBe(false)
    expect(writes(w)[before]![0]!.kind).toBe('task')
  }
})

test('task creation keeps the stage selected before its native tool runs', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('before', 'active', { session: SESSION })] })
  w.state.terminal.TaskCreate = () => {
    w.state.read = readDoc({ stages: [stage('after', 'active', { session: SESSION })] })
    return created
  }
  await start($)
  const ran = await $.tool.call({ tool: 'TaskCreate', subject: 'Task', description: '' } as never)
  await w.clock.settle()
  expect(writes(w)[0]![0]!.stage).toBe('before')
  expect(ran.text).toBe('native result')
})

test('a native task still runs when preparation or mirroring fails and the next capture can run', async ($, on) => {
  const w = world(on)
  w.state.terminal.TaskCreate = () => created
  await start($)
  w.state.answers.read = { exitCode: 1, doc: { outcome: 'error', errors: [] } }
  expect((await $.tool.call({ tool: 'TaskCreate', subject: 'Task', description: '' } as never)).text).toBe('native result')
  delete w.state.answers.read
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  w.state.answers.write = { exitCode: 1, doc: { outcome: 'refused', errors: [] } }
  expect((await $.tool.call({ tool: 'TaskCreate', subject: 'Task', description: '' } as never)).text).toBe('native result')
  delete w.state.answers.write
  await $.tool.call({ tool: 'mcp__loft__yard_write', batch: [{ id: 'later', kind: 'note', stage: 'shape', markdown: 'Next' }] })
  expect(writes(w).at(-1)![0]!.id).toBe('later')
})

test('waiting for a plan decision does not hold the capture lane', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  const decision = deferred<{ result: object }>()
  w.state.terminal.ExitPlanMode = () => decision.promise
  w.state.terminal.TaskCreate = () => created
  await start($)
  const plan = $.tool.call({ tool: 'ExitPlanMode', plan: '# Plan' } as never)
  await w.clock.settle()
  const task = $.tool.call({ tool: 'TaskCreate', subject: 'Task', description: '' } as never)
  await w.clock.settle()
  const capturedWhileWaiting = writes(w).map(batch => batch[0]!.kind)
  decision.resolve({ result: {} })
  await Promise.all([plan, task])
  expect(capturedWhileWaiting).toEqual(['plan', 'task'])
})

test('a delayed main agent capture leaves a subagent free to capture its own task', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION }), stage('dig', 'active', { session: SESSION, agent: 'a1', agentType: 'Explore' })] })
  const native = deferred<typeof created>()
  w.state.terminal.TaskCreate = e => e.agentId ? created : native.promise
  await start($)
  const main = $.tool.call({ tool: 'TaskCreate', subject: 'Main', description: '' } as never)
  await w.clock.settle()
  const sub = $.tool.call({ tool: 'TaskCreate', subject: 'Sub', description: '', agentId: 'a1' } as never)
  await w.clock.settle()
  const independent = writes(w).map(batch => batch[0]!.stage)
  native.resolve(created)
  await Promise.all([main, sub])
  expect(independent).toEqual(['dig'])
})

const INTERRUPTING = {
  name: 'interrupting', tier: 'prepend' as const,
  register(on: Parameters<typeof world>[0]) {
    on('tool.call', { tool: 'TaskCreate' }, async ($, e, next) => {
      if (e.subject !== 'Interrupted') return next(e)
      void next(e).catch(() => {})
      await $.clock.sleep(60000)
      return { deny: 'Interrupted by the person.' }
    })
  },
}

test('an interrupted native task releases its capture lane without a late mirror', { plugins: [INTERRUPTING] }, async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  const native = deferred<typeof created>()
  w.state.terminal.TaskCreate = e => e.subject === 'Interrupted' ? native.promise : created
  await start($)
  const interrupted = $.tool.call({ tool: 'TaskCreate', subject: 'Interrupted', description: '' } as never)
  await w.clock.settle()
  await w.clock.advance(60000)
  expect((await interrupted).deny).toBe('Interrupted by the person.')
  let returned = false
  const later = $.tool.call({ tool: 'TaskCreate', subject: 'Later', description: '' } as never).then(r => { returned = true; return r })
  await w.clock.settle()
  const released = returned
  native.resolve(created)
  await later
  await w.clock.settle()
  expect(released).toBe(true)
  expect(writes(w).map(batch => batch[0]!.title)).toEqual(['Later'])
})

test('a new session has its own capture lane while an old native task finishes', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('before', 'active', { session: SESSION })] })
  const native = deferred<typeof created>()
  w.state.terminal.TaskCreate = e => e.subject === 'Old' ? native.promise : created
  await start($)
  const old = $.tool.call({ tool: 'TaskCreate', subject: 'Old', description: '' } as never)
  await w.clock.settle()
  await $.session.end({ reason: 'clear', sessionId: SESSION } as never)
  w.state.session = 'session-2'
  w.state.read = readDoc({ stages: [stage('after', 'active', { session: 'session-2' })] })
  await $.classic.SessionStart({ source: 'clear' })
  const fresh = $.tool.call({ tool: 'TaskCreate', subject: 'New', description: '' } as never)
  await w.clock.settle()
  const independent = writes(w).map(batch => batch[0]!.stage)
  native.resolve(created)
  await Promise.all([old, fresh])
  expect(independent).toEqual(['after'])
  expect(writes(w).map(batch => batch[0]!.stage)).toEqual(['after', 'before'])
})

test('an interrupted preparation cannot write a late mirror after a later capture', { plugins: [INTERRUPTING] }, async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  w.state.terminal.TaskCreate = () => created
  await start($)
  const blocked = deferred<void>()
  w.state.reading = blocked.promise
  const interrupted = $.tool.call({ tool: 'TaskCreate', subject: 'Interrupted', description: '' } as never)
  await w.clock.settle()
  await w.clock.advance(60000)
  await interrupted
  w.state.reading = Promise.resolve()
  await $.tool.call({ tool: 'TaskCreate', subject: 'Later', description: '' } as never)
  blocked.resolve()
  await w.clock.settle()
  expect(writes(w).map(batch => batch[0]!.title)).toEqual(['Later'])
})

test('cancelling a queued capture never lets a later call overtake its active predecessor', { plugins: [INTERRUPTING] }, async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  const native = deferred<typeof created>()
  w.state.terminal.TaskCreate = e => e.subject === 'First' ? native.promise : created
  await start($)
  const first = $.tool.call({ tool: 'TaskCreate', subject: 'First', description: '' } as never)
  await w.clock.settle()
  const interrupted = $.tool.call({ tool: 'TaskCreate', subject: 'Interrupted', description: '' } as never)
  await w.clock.settle()
  await w.clock.advance(60000)
  await interrupted
  const later = $.tool.call({ tool: 'TaskCreate', subject: 'Later', description: '' } as never)
  await w.clock.settle()
  const overtook = writes(w).length > 0
  native.resolve(created)
  await Promise.all([first, later])
  expect(overtook).toBe(false)
  expect(writes(w).map(batch => batch[0]!.title)).toEqual(['First', 'Later'])
})
