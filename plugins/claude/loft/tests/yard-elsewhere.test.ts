import { expect, test } from 'claude-code/testing'

import { SESSION, spawn, start, TREE, world, yardRuns, type Engine } from './yard-world'

/** What `loft yard protocol` reports of a working tree that does not use Loft. */
const OUTSIDE = { usesLoft: false }
/** A directory of a repository that does not use Loft. */
const PLAIN = '/work/plain'

const PROTOCOL = ['loft', 'yard', 'protocol', '--format', 'json']
const TOOLS = ['yard_read', 'yard_write', 'yard_wait', 'yard_withdraw', 'yard_close', 'yard_published']
const QUESTIONS = [{ question: 'Which?', header: 'Pick', multiSelect: false, options: [{ label: 'A', description: '' }, { label: 'B', description: '' }] }]

/** Claude Code's own drawing of the question dialog, as a handle the test can tell apart. */
const ASKING = { surface: 'terminal' as const, component: 'AskUserQuestion' as const, requestId: 'ask-1', props: { tool: 'AskUserQuestion', questions: QUESTIONS } }

/** One model request of the turn `turnId`, for the subagent `agentId` where one is named. */
async function step($: Engine, turnId: string, agentId?: string) {
  for await (const _ of $.turn.step({ turnId, index: 0, model: 'claude-test-1', messageCount: 1, ...(agentId ? { agentId } : {}) } as never)) void _
}

test('the mod learns whether the working tree uses Loft from loft yard protocol, run in the directory the session starts in', async ($, on) => {
  const w = world(on, { workingTree: OUTSIDE })
  w.state.cwd = PLAIN
  await start($, PLAIN)
  expect(w.state.runs.map(run => ({ argv: run.argv, cwd: run.cwd }))).toEqual([{ argv: PROTOCOL, cwd: PLAIN }])
  expect(w.state.tools).toEqual([])
})

test('in a working tree that does not use Loft, no Yard tool and no /yard-pane command is registered, and a run of the command opens no pane', async ($, on) => {
  const w = world(on, { workingTree: OUTSIDE })
  await start($)
  expect(w.state.tools).toEqual([])
  expect(w.state.commands).toEqual([])
  await $.command.run({ command: 'yard-pane', args: '' } as never)
  await w.clock.advance(3000)
  expect(w.state.opened).toEqual([])
  expect(w.state.panes).toEqual([])
})

test('in a working tree that does not use Loft, no loft yard serve runs and no page server hears of the session, even where one answers', async ($, on) => {
  const w = world(on, { workingTree: OUTSIDE, serve: 'serving' })
  w.state.answersBeat = true
  await start($)
  await w.clock.advance(60000)
  expect(w.state.runs.filter(run => run.argv.includes('serve'))).toEqual([])
  expect(w.state.heard).toEqual([])
  expect(w.state.posts).toEqual([])
})

test('in a working tree that does not use Loft, no event of the session is posted', async ($, on) => {
  const w = world(on, { workingTree: OUTSIDE })
  w.state.answersBeat = true
  on('turn.step', async function* ($, e) {
    return { turnId: e.turnId, index: e.index, answer: 'done', toolUses: [{ name: 'Read', id: 'u1', input: {} }], stopReason: 'tool_use', usage: { input_tokens: 10, output_tokens: 5, cache_read_input_tokens: 100, cache_creation_input_tokens: 20, model: 'claude-test-1' } } as never
  })
  await start($)
  await $.turn.start({ text: 'go', turnId: 't1' })
  await step($, 't1')
  await spawn($, w, 'a9', 'Explore')
  await step($, 'a9-turn', 'a9')
  await $.tool.call({ tool: 'Read', file_path: `${TREE}/README.md`, agentId: 'a9', tool_use_id: 'u1' } as never)
  await $.turn.complete({ agentId: 'a9', turnId: 'a9-turn', answer: '', durationMs: 1, reason: 'answer', isAborted: false } as never)
  await $.tool.call({ tool: 'Edit', file_path: `${TREE}/spec/a.yaml`, old_string: 'a', new_string: 'b', tool_use_id: 'u2' } as never)
  await $.tool.call({ tool: 'Bash', command: 'loft check --fail-on warning', tool_use_id: 'u3' } as never)
  await $.turn.complete({ turnId: 't1', answer: '', durationMs: 1, reason: 'answer', isAborted: false } as never)
  await w.clock.advance(15000)
  expect(w.state.posts).toEqual([])
  expect(w.state.heard).toEqual([])
})

test("in a working tree that does not use Loft, AskUserQuestion and ExitPlanMode show Claude Code's own dialogs alone, and no Yard Log is opened", async ($, on) => {
  const w = world(on, { workingTree: OUTSIDE })
  on('ui.render', { component: 'AskUserQuestion' }, () => ({ type: 'engine', ref: 0 }) as const)
  w.state.terminal.AskUserQuestion = e => ({ result: { questions: e.questions, answers: { 'Which?': 'A' } }, text: 'ok' })
  await start($)
  const asked = await $.tool.call({ tool: 'AskUserQuestion', questions: QUESTIONS })
  await $.tool.call({ tool: 'ExitPlanMode', plan: '# P', planFilePath: '/p/plan.md' } as never)
  expect(asked.result).toEqual({ questions: QUESTIONS, answers: { 'Which?': 'A' } })
  expect(w.state.calls.map(call => call.tool)).toEqual(['AskUserQuestion', 'ExitPlanMode'])
  expect(w.state.calls[0]).toMatchObject({ questions: QUESTIONS })
  expect(w.state.calls[1]).toMatchObject({ plan: '# P', planFilePath: '/p/plan.md' })
  // The kit may hand Claude Code's drawing back under a handle of its own numbering.
  const drawn = await $.ui.render(ASKING)
  expect(drawn).toMatchObject({ type: 'engine' })
  expect(Object.keys(drawn as object).sort()).toEqual(['ref', 'type'])
  expect(yardRuns(w)).toEqual(['protocol --format json'])
})

test('in a working tree that does not use Loft, the mod shows no notice, not even of a yardListen entry it would name', { options: { yardListen: 'machine.example.ts.net' } }, async ($, on) => {
  const w = world(on, { workingTree: OUTSIDE, protocol: 4 })
  await start($)
  await w.clock.advance(10000)
  expect(w.state.toasts).toEqual([])
})

test('in a working tree that does not use Loft, a loft of a protocol the plugin does not speak still tells the person once why Yard Client is off', async ($, on) => {
  const w = world(on, { workingTree: OUTSIDE, protocol: 5 })
  await start($)
  expect(w.state.tools).toEqual([])
  expect(w.state.toasts).toHaveLength(1)
  expect(w.state.runs.map(run => run.argv)).toEqual([PROTOCOL])
})

test('in a working tree that does not use Loft, the mod runs no loft command but loft yard protocol, whatever the session does', async ($, on) => {
  const w = world(on, { workingTree: OUTSIDE })
  w.state.answersBeat = true
  w.state.terminal.TaskCreate = () => ({ result: { task: { id: '3' } }, text: 'ok' })
  await start($)
  await $.turn.start({ text: 'go', turnId: 't1' })
  for (const tool of ['mcp__loft__yard_read', 'mcp__loft__yard_write', 'mcp__loft__yard_wait', 'mcp__loft__yard_withdraw', 'mcp__loft__yard_close', 'mcp__loft__yard_published']) {
    await $.tool.call({ tool, batch: [], submission: 'sub-1', branch: 'feat/shelf' } as never)
  }
  await $.tool.call({ tool: 'TaskCreate', subject: 'Task', description: '' } as never)
  await $.tool.call({ tool: 'TaskUpdate', taskId: '3', status: 'in_progress' } as never)
  await $.tool.call({ tool: 'AskUserQuestion', questions: QUESTIONS })
  await $.tool.call({ tool: 'ExitPlanMode', plan: '# P', planFilePath: '/p/plan.md' } as never)
  await $.tool.call({ tool: 'Bash', command: 'loft yard read', tool_use_id: 'u1' } as never)
  // A subagent's work and its end, which release its tasks while Yard Client is on.
  await spawn($, w, 'a9', 'Explore')
  await $.tool.call({ tool: 'TaskUpdate', taskId: '3', status: 'completed', agentId: 'a9', metadata: { yardTask: 'shared' } } as never)
  await $.turn.complete({ agentId: 'a9', turnId: 'a9-turn', answer: '', durationMs: 1, reason: 'answer', isAborted: false } as never)
  // An interrupted turn, which releases the main agent's tasks while Yard Client is on.
  await $.turn.complete({ turnId: 't1', answer: '', durationMs: 1, reason: 'aborted', isAborted: true } as never)
  await $.command.run({ command: 'yard-pane', args: '' } as never)
  // `/clear`: the session that follows would record whom it continues while Yard Client is on.
  await $.session.end({ reason: 'clear', sessionId: SESSION } as never)
  w.state.session = 'session-2'
  await $.turn.start({ text: 'again', turnId: 't2' })
  await $.tool.call({ tool: 'mcp__loft__yard_read' })
  await w.clock.advance(30000)
  await $.session.end({ reason: 'other', sessionId: 'session-2' } as never)
  await w.clock.settle()
  expect(w.state.runs.map(run => run.argv)).toEqual([PROTOCOL])
  expect(w.state.toasts).toEqual([])
})

test('a loft that does not report whether the working tree uses Loft leaves Yard Client on, as before', async ($, on) => {
  const w = world(on, { workingTree: 'unreported' })
  await start($)
  expect(w.state.tools.map(tool => tool.name)).toEqual(TOOLS)
  expect(w.state.commands.map(command => command.name)).toEqual(['yard-pane'])
  expect(w.state.toasts).toEqual([])
  await w.clock.advance(1)
  expect(w.state.runs.filter(run => run.argv.includes('serve'))).toHaveLength(1)
})

test('a working tree that uses Loft turns Yard Client on, whatever shows it', async ($, on) => {
  const w = world(on, { workingTree: { usesLoft: true, by: 'config' } })
  await start($)
  expect(w.state.tools.map(tool => tool.name)).toEqual(TOOLS)
  expect(w.state.toasts).toEqual([])
})
