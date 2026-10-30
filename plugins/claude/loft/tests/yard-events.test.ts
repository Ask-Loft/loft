import { expect, test } from 'claude-code/testing'

import { flush, keep, MOST, outboxOf } from '../hooks/adapter/outbox'
import { block, deferred, readDoc, SESSION, spawn, stage, start, TREE, world, yardRuns, type Engine, type World } from './yard-world'

const events = (w: World) => w.state.posts.flatMap(post => post.events)
const kinds = (w: World) => events(w).map(event => String(event.kind))
const USAGE = { input_tokens: 10, output_tokens: 5, cache_read_input_tokens: 100, cache_creation_input_tokens: 20 }

test("each event names the session, its directory, its time to the second, its kind and the host, and is posted to the page server's events with the token", async ($, on) => {
  const w = world(on)
  await start($)
  await $.turn.start({ text: 'go', turnId: 't1' })
  await w.clock.settle()
  const [post] = w.state.posts
  expect(post!.url).toBe('http://127.0.0.1:7411/api/events')
  expect(post!.headers).toEqual({ 'Content-Type': 'application/json', Authorization: 'Bearer token-1' })
  expect(post!.events).toEqual([{ session: SESSION, dir: TREE, at: '2026-10-22T09:30:00Z', host: 'claude-code', kind: 'turn.start', turn: 't1' }])
})

test('a post carries no Origin, so the page server tells the mod from a page', async ($, on) => {
  const w = world(on)
  await start($)
  await $.turn.start({ text: 'go', turnId: 't1' })
  await w.clock.settle()
  expect(Object.keys(w.state.posts[0]!.headers).map(name => name.toLowerCase())).not.toContain('origin')
})

test("a tool call's start and end are posted, the end saying whether it failed, an edit whether it is under spec/, and a shell command the loft subcommands it runs", async ($, on) => {
  const w = world(on)
  await start($)
  await $.tool.call({ tool: 'Edit', file_path: `${TREE}/spec/products/a.yaml`, old_string: 'a', new_string: 'b', tool_use_id: 'u1' } as never)
  await $.tool.call({ tool: 'Write', file_path: `${TREE}/src/main.rs`, content: '', tool_use_id: 'u2' } as never)
  await $.tool.call({ tool: 'Bash', command: 'loft check --fail-on warning && cargo test', tool_use_id: 'u3' } as never)
  w.state.terminal.Grep = () => ({ deny: 'no' })
  await $.tool.call({ tool: 'Grep', pattern: 'x', tool_use_id: 'u4' } as never)
  await w.clock.settle()
  const shapes = events(w).filter(event => String(event.kind).startsWith('tool.')).map(({ kind, tool, call, spec, loft, failed }) => ({ kind, tool, call, spec, loft, failed }))
  expect(shapes).toEqual([
    { kind: 'tool.start', tool: 'Edit', call: 'u1', spec: true, loft: undefined, failed: undefined },
    { kind: 'tool.end', tool: 'Edit', call: 'u1', spec: undefined, loft: undefined, failed: false },
    { kind: 'tool.start', tool: 'Write', call: 'u2', spec: false, loft: undefined, failed: undefined },
    { kind: 'tool.end', tool: 'Write', call: 'u2', spec: undefined, loft: undefined, failed: false },
    { kind: 'tool.start', tool: 'Bash', call: 'u3', spec: undefined, loft: ['check'], failed: undefined },
    { kind: 'tool.end', tool: 'Bash', call: 'u3', spec: undefined, loft: undefined, failed: false },
    { kind: 'tool.start', tool: 'Grep', call: 'u4', spec: undefined, loft: undefined, failed: undefined },
    { kind: 'tool.end', tool: 'Grep', call: 'u4', spec: undefined, loft: undefined, failed: true },
  ])
})

test("a subagent's events name it by its id and type, from its spawn to its turn's end", async ($, on) => {
  const w = world(on)
  await start($)
  await spawn($, w, 'a9', 'Explore')
  await $.tool.call({ tool: 'Read', file_path: '/x', agentId: 'a9', tool_use_id: 'u1' } as never)
  await $.turn.complete({ agentId: 'a9', turnId: 'a9-turn', answer: '', durationMs: 1, reason: 'answer', isAborted: false } as never)
  await w.clock.settle()
  const of = events(w).filter(event => ['subagent.start', 'tool.start', 'subagent.end'].includes(String(event.kind)))
  expect(of.map(event => [event.kind, event.agent])).toEqual([
    ['subagent.start', { id: 'a9', type: 'Explore' }],
    ['tool.start', { id: 'a9', type: 'Explore' }],
    ['subagent.end', { id: 'a9', type: 'Explore' }],
  ])
})

test("a turn's end carries its tokens", async ($, on) => {
  const w = world(on)
  await start($)
  await $.turn.complete({ turnId: 't1', answer: '', usage: USAGE } as never)
  await w.clock.settle()
  expect(events(w)[0]).toMatchObject({ kind: 'turn.end', turn: 't1', tokens: { input: 10, cacheRead: 100, cacheWrite: 20, output: 5 } })
})

test("the mod posts no plan mode: the plugin's command hooks hear it from the permission mode", async ($, on) => {
  const w = world(on)
  await start($)
  await $.tool.call({ tool: 'EnterPlanMode' } as never)
  await $.turn.start({ text: 'go', turnId: 't1' })
  await w.clock.settle()
  expect(kinds(w).filter(kind => kind.startsWith('plan.'))).toEqual([])
})

test("the mod's own dialogs and commands are not posted as the agent's tool calls", async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({
    stages: [{ ...stage('shape', 'active', { session: SESSION }), blocks: [block('q1', 'question', { prompt: 'Pick?', options: [{ label: 'A' }, { label: 'B' }] })] }],
    submissions: [{ agent: { session: SESSION }, open: [{ key: 'sub-7', blocks: ['k-q1'], state: 'open' }] }],
  })
  w.state.terminal.AskUserQuestion = () => {
    w.state.page.resolve({ outcome: 'answered', errors: [], blocks: [] })
    return new Promise(() => {})
  }
  await start($)
  await $.tool.call({ tool: 'mcp__loft__yard_wait', submission: 'sub-7', tool_use_id: 'u1' } as never)
  await w.clock.settle()
  expect(events(w).map(event => event.tool)).toEqual(['mcp__loft__yard_wait', 'mcp__loft__yard_wait'])
})

test('events wait while the page server does not take them, and go in their order once it does', async ($, on) => {
  const w = world(on)
  await start($)
  w.state.postStatus = 503
  await $.turn.start({ text: 'a', turnId: 't1' })
  await $.turn.start({ text: 'b', turnId: 't2' })
  await w.clock.settle()
  w.state.posts.length = 0
  w.state.postStatus = 200
  await w.clock.advance(5000)
  expect(events(w).map(event => event.turn)).toEqual(['t1', 't2'])
})

test('the events kept while no page server answered are posted, in order, to the page server on the port loft yard serve reports', async ($, on) => {
  const w = world(on, { serve: 'running' })
  w.state.servedPort = 7412
  w.state.answersBeat = true
  await start($)
  await $.turn.start({ text: '', turnId: 't1' })
  await w.clock.advance(5000)
  expect(w.state.posts).toEqual([])
  w.state.reportedPort = 7412
  await $.turn.start({ text: '', turnId: 't2' })
  await w.clock.advance(10000)
  expect(events(w).filter(event => event.kind === 'turn.start').map(event => event.turn)).toEqual(['t1', 't2'])
  expect(new Set(w.state.posts.map(post => post.url))).toEqual(new Set(['http://127.0.0.1:7412/api/events']))
  expect(events(w).some(event => 'dropped' in event)).toBe(false)
})

test('past 1,000 events kept the oldest is dropped, and the next post says how many were', async ($, on) => {
  const w = world(on)
  await start($)
  w.state.postStatus = 503
  for (let n = 0; n < 1003; n++) await $.turn.start({ text: '', turnId: `t${n}` })
  await w.clock.settle()
  w.state.posts.length = 0
  w.state.postStatus = 200
  await w.clock.advance(5000)
  const sent = events(w)
  expect(sent).toHaveLength(1000)
  expect(sent[0]).toMatchObject({ turn: 't3', dropped: 3 })
  expect(sent[1]!.dropped).toBeUndefined()
})

test('no event is posted before the token exists, and none is lost meanwhile', async ($, on) => {
  const w = world(on)
  w.state.hasToken = false
  await start($)
  await $.turn.start({ text: '', turnId: 't1' })
  await w.clock.settle()
  expect(w.state.posts).toEqual([])
  w.state.hasToken = true
  await w.clock.advance(5000)
  expect(events(w).map(event => event.turn)).toEqual(['t1'])
})

test('with Yard Client off, no event is posted', async ($, on) => {
  const w = world(on, { protocol: 'none' })
  await start($)
  await $.turn.start({ text: '', turnId: 't1' })
  await $.tool.call({ tool: 'Bash', command: 'loft check' })
  await w.clock.advance(10000)
  expect(w.state.posts).toEqual([])
})

const taskRuns = (w: World) => yardRuns(w).filter(run => run.startsWith('task ')).map(run => run.split(' --session ')[0])

test("a task the agent makes is mirrored as a task block of its working stage, and the task's updates as changes of it", async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  w.state.terminal.TaskCreate = () => ({ result: { task: { id: '3' } }, text: 'ok' })
  await start($)
  await $.tool.call({ tool: 'TaskCreate', subject: 'Write the tests', description: '' } as never)
  await w.clock.settle()
  await $.tool.call({ tool: 'TaskUpdate', taskId: '3', status: 'in_progress' } as never)
  await w.clock.settle()
  await $.tool.call({ tool: 'TaskUpdate', taskId: '3', status: 'completed', subject: 'Write the kit tests' } as never)
  await w.clock.settle()
  const written = w.state.runs.filter(run => run.argv.includes('write'))
  expect(written.map(run => JSON.parse(run.stdin!))).toEqual([
    [{ id: 'task-session1-3', kind: 'task', stage: 'shape', title: 'Write the tests', state: 'open' }],
  ])
  expect(written[0]!.argv).not.toContain('--based-on')
  expect(taskRuns(w)).toEqual([
    'task task-session1-3 --mirror --state active',
    'task task-session1-3 --mirror --state done --title=Write the kit tests',
  ])
})

test("an unbound task mirrored into a working stage the agent does not own is assigned to that agent", async ($, on) => {
  const w = world(on, { protocol: 2 })
  w.state.agents.push({ id: 'a9', type: 'Explore' })
  w.state.read = readDoc({ stages: [stage('shape', 'active', { host: 'claude-code', session: SESSION })] })
  w.state.terminal.TaskCreate = () => ({ result: { task: { id: '4' } }, text: 'ok' })
  await start($)
  await $.tool.call({ tool: 'TaskCreate', subject: 'Dig', description: '', agentId: 'a9' } as never)
  await $.tool.call({ tool: 'TaskCreate', subject: 'Shape', description: '' } as never)
  await w.clock.settle()
  const written = w.state.runs.filter(run => run.argv.includes('write')).map(run => JSON.parse(run.stdin!))
  expect(written).toEqual([
    [{ id: 'task-a9-4', kind: 'task', stage: 'shape', title: 'Dig', state: 'open',
      assignee: { host: 'claude-code', session: SESSION, agent: 'a9', agentType: 'Explore' } }],
    [{ id: 'task-session1-4', kind: 'task', stage: 'shape', title: 'Shape', state: 'open' }],
  ])
})

test('a task created with the type fix in its metadata is mirrored as a fix, and one with no type or another type as regular', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  let id = 0
  w.state.terminal.TaskCreate = () => ({ result: { task: { id: String(++id) } }, text: 'ok' })
  await start($)
  await $.tool.call({ tool: 'TaskCreate', subject: 'Mend the gutter', description: '', metadata: { type: 'fix' } } as never)
  await $.tool.call({ tool: 'TaskCreate', subject: 'Plan', description: '' } as never)
  await $.tool.call({ tool: 'TaskCreate', subject: 'Tidy', description: '', metadata: { type: 'chore' } } as never)
  await w.clock.settle()
  const written = w.state.runs.filter(run => run.argv.includes('write')).map(run => JSON.parse(run.stdin!))
  expect(written).toEqual([
    [{ id: 'task-session1-1', kind: 'task', stage: 'shape', title: 'Mend the gutter', state: 'open', type: 'fix' }],
    [{ id: 'task-session1-2', kind: 'task', stage: 'shape', title: 'Plan', state: 'open' }],
    [{ id: 'task-session1-3', kind: 'task', stage: 'shape', title: 'Tidy', state: 'open' }],
  ])
})

test("a task's update sets its type from its metadata, null makes it regular, and an update keeps the type and detail it leaves alone", async ($, on) => {
  const w = world(on)
  await start($)
  await $.tool.call({ tool: 'TaskUpdate', taskId: '3', metadata: { type: 'fix' } } as never)
  await $.tool.call({ tool: 'TaskUpdate', taskId: '3', status: 'in_progress' } as never)
  await $.tool.call({ tool: 'TaskUpdate', taskId: '3', metadata: { type: null } } as never)
  await $.tool.call({ tool: 'TaskUpdate', taskId: '3', description: 'Why not' } as never)
  await w.clock.settle()
  expect(taskRuns(w)).toEqual([
    'task task-session1-3 --mirror --type fix',
    'task task-session1-3 --mirror --state active',
    'task task-session1-3 --mirror --type regular',
    'task task-session1-3 --mirror --detail=Why not',
  ])
})

test('an agent with no working stage has no task mirrored, and a failed task call mirrors nothing', async ($, on) => {
  const w = world(on)
  w.state.terminal.TaskCreate = () => ({ result: { task: { id: '3' } }, text: 'ok' })
  await start($)
  await $.tool.call({ tool: 'TaskCreate', subject: 'A', description: '' } as never)
  await w.clock.settle()
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  w.state.terminal.TaskCreate = () => ({ deny: 'no' })
  await $.tool.call({ tool: 'TaskCreate', subject: 'B', description: '' } as never)
  w.state.terminal.TaskUpdate = () => ({ deny: 'no' })
  await $.tool.call({ tool: 'TaskUpdate', taskId: '3', status: 'completed' } as never)
  await w.clock.settle()
  expect(w.state.runs.filter(run => run.argv.includes('write'))).toEqual([])
  expect(taskRuns(w)).toEqual([])
})

test("the agent's task call waits until its mirror has been attempted", async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  w.state.terminal.TaskCreate = () => ({ result: { task: { id: '3' } }, text: 'ok' })
  await start($)
  const read = deferred<void>()
  w.state.reading = read.promise
  let returned = false
  const pending = $.tool.call({ tool: 'TaskCreate', subject: 'Write the tests', description: '' } as never).then(ran => { returned = true; return ran })
  await w.clock.settle()
  const returnedBeforeRead = returned
  read.resolve()
  const ran = await pending
  await w.clock.settle()
  expect(returnedBeforeRead).toBe(false)
  expect(ran.text).toBe('ok')
  expect(w.state.runs.filter(run => run.argv.includes('write'))).toHaveLength(1)
})

const STEP = { turnId: 't1', index: 0, model: 'claude-test-1', messageCount: 1 }
const RESPONSE = {
  turnId: 't1',
  index: 0,
  answer: 'done',
  toolUses: [{ name: 'Read', id: 'u1', input: {} }],
  stopReason: 'tool_use',
  usage: { ...USAGE, model: 'claude-test-1' },
}

const step = async ($: Engine, agentId?: string) => {
  for await (const _ of $.turn.step({ ...STEP, ...(agentId ? { agentId } : {}) } as never)) void _
}

test("a model request's event is posted once its response ends, with the model, the turn, the tools it asked for and its tokens; a subagent's start is posted as it is spawned, with its model", async ($, on) => {
  const w = world(on)
  on('turn.step', async function* () {
    return RESPONSE as never
  })
  await start($)
  await spawn($, w, 'a9', 'Explore')
  await w.clock.settle()
  expect(kinds(w)).toEqual(['subagent.start'])
  await step($, 'a9')
  await step($)
  await w.clock.settle()
  expect(events(w).map(({ kind, agent, model, turn, tools, tokens }) => ({ kind, agent, model, turn, tools, tokens }))).toEqual([
    { kind: 'subagent.start', agent: { id: 'a9', type: 'Explore' }, model: 'claude-test-1', turn: undefined, tools: undefined, tokens: undefined },
    { kind: 'request', agent: { id: 'a9', type: 'Explore' }, model: 'claude-test-1', turn: 't1', tools: ['Read'], tokens: { input: 10, cacheRead: 100, cacheWrite: 20, output: 5 } },
    { kind: 'request', agent: undefined, model: 'claude-test-1', turn: 't1', tools: ['Read'], tokens: { input: 10, cacheRead: 100, cacheWrite: 20, output: 5 } },
  ])
})

test("no event holds the prompt, the agent's text, a tool call's input or its result", async ($, on) => {
  const w = world(on)
  on('turn.step', async function* () {
    return RESPONSE as never
  })
  await start($)
  await $.turn.start({ text: 'a secret prompt', turnId: 't1' })
  await step($)
  await $.tool.call({ tool: 'Bash', command: 'echo secret && loft check', tool_use_id: 'u1' } as never)
  await $.tool.call({ tool: 'Edit', file_path: `${TREE}/spec/secret.yaml`, old_string: 'secret', new_string: 'secret', tool_use_id: 'u2' } as never)
  await w.clock.settle()
  const allowed = new Set(['session', 'dir', 'at', 'host', 'agent', 'kind', 'turn', 'model', 'tools', 'tokens', 'tool', 'call', 'spec', 'loft', 'failed'])
  for (const event of events(w)) for (const field of Object.keys(event)) expect(allowed.has(field), field).toBe(true)
  expect(JSON.stringify(events(w))).not.toContain('secret')
})

test("the agent's work goes on while the page server has yet to answer", async ($, on) => {
  const w = world(on)
  w.state.answering = new Promise(() => {})
  await start($)
  const ran = await $.tool.call({ tool: 'Bash', command: 'ls', tool_use_id: 'u1' } as never)
  expect(ran.text).toBe('ok')
})


test("after /clear a task the new session makes is a block of its own, and the mirror's reads leave the last-read revision as it was", async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  w.state.terminal.TaskCreate = () => ({ result: { task: { id: '1' } }, text: 'ok' })
  await start($)
  await $.tool.call({ tool: 'TaskCreate', subject: 'Before', description: '' } as never)
  await w.clock.settle()
  await $.session.end({ reason: 'clear', sessionId: SESSION } as never)
  w.state.session = 'session-2'
  await $.turn.start({ text: 'go', turnId: 't2' })
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: 'session-2' })] })
  const before = w.state.runs.length
  await $.tool.call({ tool: 'TaskCreate', subject: 'After', description: '' } as never)
  await w.clock.settle()
  const ids = w.state.runs.filter(run => run.argv.includes('write')).map(run => (JSON.parse(run.stdin!) as { id: string }[])[0]!.id)
  expect(ids).toEqual(['task-session1-1', 'task-session2-1'])
  const mirrorReads = w.state.runs.slice(before).filter(run => run.argv.includes('read'))
  expect(mirrorReads.length).toBeGreaterThan(0)
  for (const run of mirrorReads) expect(run.argv).not.toContain('--mark-read')
})

/** An outbox whose posts the test answers one at a time, holding each until it is answered. */
function heldOutbox() {
  const box = outboxOf(7411)
  box.token = 'token-1'
  const posts: { events: Record<string, unknown>[]; answer: (status: number) => void }[] = []
  const notices = new Map<number, ReturnType<typeof deferred<void>>>()
  const posted = (index: number) => {
    if (posts.length > index) return Promise.resolve()
    const notice = deferred<void>()
    notices.set(index, notice)
    return notice.promise
  }
  const $ = {
    http: {
      fetch: (_url: string, init: { body: string }) =>
        new Promise(answered => {
          posts.push({ events: JSON.parse(init.body), answer: status => answered({ ok: status < 300, status }) })
          notices.get(posts.length - 1)?.resolve()
        }),
    },
  } as unknown as Parameters<typeof flush>[0]
  return { box, posts, posted, flushing: () => flush($, box) }
}

test('an event kept while a full post is under way is posted after it, and is not counted as dropped', async () => {
  const { box, posts, posted, flushing } = heldOutbox()
  for (let n = 0; n < MOST; n++) keep(box, { kind: 'turn.start', turn: `t${n}` })
  const flushed = flushing()
  await posted(0)
  keep(box, { kind: 'turn.start', turn: 'late' })
  posts[0]!.answer(200)
  await posted(1)
  posts[1]!.answer(200)
  await flushed
  expect(posts[0]!.events).toHaveLength(MOST)
  expect(posts[1]!.events).toEqual([{ kind: 'turn.start', turn: 'late' }])
  expect(box.queue).toEqual([])
  expect(box.dropped).toBe(0)
})

test('a full post the page server does not take keeps the newest 1,000 events, and counts the oldest dropped', async () => {
  const { box, posts, posted, flushing } = heldOutbox()
  for (let n = 0; n < MOST; n++) keep(box, { kind: 'turn.start', turn: `t${n}` })
  const flushed = flushing()
  await posted(0)
  keep(box, { kind: 'turn.start', turn: 'late' })
  posts[0]!.answer(503)
  await flushed
  expect(box.queue).toHaveLength(MOST)
  expect(box.queue[0]).toMatchObject({ turn: 't1' })
  expect(box.queue.at(-1)).toMatchObject({ turn: 'late' })
  expect(box.dropped).toBe(1)
})
