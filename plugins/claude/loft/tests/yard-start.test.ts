import { expect, test } from 'claude-code/testing'

import { flagOf, HOME, readDoc, SESSION, start, stage, TREE, world, yardRuns } from './yard-world'

test('a loft that speaks the yard protocol turns Yard Client on: its six tools are registered at the start', async ($, on) => {
  const w = world(on)
  await $.session.start({ cwd: TREE, surface: 'terminal', isInteractive: true })
  expect(w.state.tools.map(tool => tool.name)).toEqual(['yard_read', 'yard_write', 'yard_wait', 'yard_withdraw', 'yard_close', 'yard_published'])
  expect(yardRuns(w)[0]).toBe('protocol --format json')
  expect(w.state.toasts).toEqual([])
})

test('a loft with no yard commands leaves Yard Client off, registers no tool and says once which to update', async ($, on) => {
  const w = world(on, { protocol: 'none' })
  await start($)
  await $.classic.SessionStart({ source: 'clear' })
  expect(w.state.tools).toEqual([])
  expect(w.state.toasts).toHaveLength(1)
  expect(w.state.toasts[0]).toMatch(/Yard Client is off: the loft plugin 2\.2\.1 speaks the yard protocol 1, and `loft` runs loft .*, which has no yard commands\. To use Yard Client, bring loft up to date, or set the loft plugin's loftCommand setting\./)
  const denied = await $.tool.call({ tool: 'mcp__loft__yard_read' })
  expect(denied.deny ?? denied.text).toMatch(/Yard Client is off/)
})

test('a loft of a newer protocol than the plugin speaks leaves Yard Client off and names the plugin to update', async ($, on) => {
  const w = world(on, { protocol: 2 })
  await start($)
  expect(w.state.tools).toEqual([])
  expect(w.state.toasts[0]).toMatch(/runs loft 2\.2\.1, which speaks the yard protocol 2\. To use Yard Client, bring the loft plugin up to date/)
})

test('a loftCommand that cannot be run leaves Yard Client off and names the setting', async ($, on) => {
  const w = world(on, { protocol: 'unrunnable' })
  await start($)
  expect(w.state.tools).toEqual([])
  expect(w.state.toasts[0]).toMatch(/Yard Client is off: `loft` cannot be run .*loftCommand setting/)
})

test('with Yard Client off, the agent asks and plans in Claude Code alone', async ($, on) => {
  const w = world(on, { protocol: 'none' })
  await start($)
  w.state.runs.length = 0
  await $.tool.call({ tool: 'AskUserQuestion', questions: [{ question: 'Which?', header: 'Pick', multiSelect: false, options: [{ label: 'A', description: '' }, { label: 'B', description: '' }] }] })
  await $.tool.call({ tool: 'ExitPlanMode', plan: '# P', planFilePath: '/p/plan.md' } as never)
  expect(yardRuns(w)).toEqual([])
  expect(w.state.calls.map(call => call.tool)).toEqual(['AskUserQuestion', 'ExitPlanMode'])
})

test('the session starts with the context of its branch, from one read in its own name', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  const started = await start($)
  const reads = w.state.runs.filter(run => run.argv.includes('read'))
  expect(reads).toHaveLength(1)
  expect(flagOf(reads[0]!.argv, '--session')).toBe(SESSION)
  expect(reads[0]!.cwd).toBe(TREE)
  const text = (started.additionalContext ?? []).join('\n')
  expect(text).toContain('Yard Log of `feat/shelf`')
  expect(text).toContain('Your active stage: `shape`')
  expect(text.length).toBeLessThanOrEqual(2000)
})

test('after /clear the session is given its context again, and its first command names the session it continues', async ($, on) => {
  const w = world(on)
  await start($)
  await $.session.end({ reason: 'clear', sessionId: SESSION } as never)
  w.state.session = 'session-2'
  const again = await $.classic.SessionStart({ source: 'clear' })
  expect((again.additionalContext ?? []).join('\n')).toContain('Yard Log of `feat/shelf`')
  const reads = w.state.runs.filter(run => run.argv.includes('read'))
  expect(flagOf(reads.at(-1)!.argv, '--session')).toBe('session-2')
  expect(flagOf(reads.at(-1)!.argv, '--continues')).toBe(SESSION)
  await $.tool.call({ tool: 'mcp__loft__yard_read' })
  expect(w.state.runs.at(-1)!.argv).not.toContain('--continues')
})

test('a compaction gives no new context', async ($, on) => {
  const w = world(on)
  await start($)
  const before = w.state.runs.length
  const compacted = await $.classic.SessionStart({ source: 'compact' })
  expect(compacted.additionalContext ?? []).toEqual([])
  expect(w.state.runs.length).toBe(before)
})

test('every command Yard Client runs is loftCommand with --format json, quietly, and no other program', { options: { loftCommand: 'pnpm exec loft' } }, async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  w.state.terminal.AskUserQuestion = e => ({ result: { questions: e.questions, answers: { 'Which?': 'A' } }, text: 'ok' })
  await start($)
  await $.tool.call({ tool: 'mcp__loft__yard_read', since: 2 })
  await $.tool.call({ tool: 'mcp__loft__yard_write', batch: [{ id: 'shape', kind: 'stage', title: 'Shape', status: 'active' }] })
  await $.tool.call({ tool: 'AskUserQuestion', questions: [{ question: 'Which?', header: 'Pick', multiSelect: false, options: [{ label: 'A', description: '' }, { label: 'B', description: '' }] }] })
  await $.tool.call({ tool: 'mcp__loft__yard_close' })
  await w.clock.advance(5000)
  const subs = new Set(yardRuns(w).map(run => run.split(' ')[0]))
  expect([...subs].sort()).toEqual(['answer', 'close', 'protocol', 'read', 'serve', 'wait', 'write'])
  for (const run of w.state.runs) {
    expect(run.argv.slice(0, 4)).toEqual(['pnpm', 'exec', 'loft', 'yard'])
    expect(run.argv.slice(-2)).toEqual(['--format', 'json'])
    expect(run.env).toEqual({ LOFT_TELEMETRY: 'off', LOFT_UPDATE_CHECK: 'off' })
  }
})

test('each tool runs its command in the session directory with the base branch and the page server port', { options: { baseBranch: 'main', yardPort: 7500 } }, async ($, on) => {
  const w = world(on)
  await start($)
  w.state.cwd = `${TREE}/src`
  await $.tool.call({ tool: 'mcp__loft__yard_read' })
  const run = w.state.runs.at(-1)!
  expect(run.cwd).toBe(`${TREE}/src`)
  expect(flagOf(run.argv, '--base')).toBe('main')
  expect(flagOf(run.argv, '--port')).toBe('7500')
})

test("a tool gives the command's document to the agent, and a refusal's message as its error", async ($, on) => {
  const w = world(on)
  await start($)
  const read = await $.tool.call({ tool: 'mcp__loft__yard_read' })
  expect(JSON.parse(String(read.result))).toEqual(w.state.read)
  w.state.answers.write = { doc: { outcome: 'refused', errors: [{ message: 'an agent has one active stage at a time' }] }, exitCode: 1 }
  const refused = await $.tool.call({ tool: 'mcp__loft__yard_write', batch: [{ id: 'two', kind: 'stage' }] })
  expect(refused.deny ?? refused.text).toContain('an agent has one active stage at a time')
  const fault = await $.tool.call({ tool: 'mcp__loft__yard_write', batch: 'nope' })
  expect(fault.deny ?? fault.text).toContain('the field `batch` must be a list of objects')
})

test('a subagent calls a tool in its own name: its id and its type', async ($, on) => {
  const w = world(on)
  await start($)
  await $.classic.SubagentStart({ agent_id: 'a9', agent_type: 'Explore' } as never)
  await $.tool.call({ tool: 'mcp__loft__yard_read', agentId: 'a9' })
  const run = w.state.runs.at(-1)!
  expect(flagOf(run.argv, '--agent')).toBe('a9')
  expect(flagOf(run.argv, '--agent-type')).toBe('Explore')
  w.state.agents = [{ id: 'b2', type: 'Plan' }]
  await $.tool.call({ tool: 'mcp__loft__yard_read', agentId: 'b2' })
  expect(flagOf(w.state.runs.at(-1)!.argv, '--agent-type')).toBe('Plan')
})

test('every five seconds the mod runs loft yard serve for the session and its tree, and one that serves is not run again', async ($, on) => {
  const w = world(on, { serve: 'serving' })
  await start($)
  const serves = () => w.state.runs.filter(run => run.argv.includes('serve'))
  expect(serves()).toHaveLength(1)
  expect(serves()[0]!.argv).toEqual(['loft', 'yard', 'serve', '--session', SESSION, '--tree', TREE, '--base', 'develop', '--port', '7411', '--format', 'json'])
  await w.clock.advance(15000)
  expect(serves()).toHaveLength(1)
  w.state.serverStops.resolve()
  await w.clock.advance(5000)
  expect(serves().length).toBeGreaterThan(1)
})

test('a page server another session runs is heard from at every beat', async ($, on) => {
  const w = world(on, { serve: 'running' })
  await start($)
  await w.clock.advance(10000)
  expect(w.state.runs.filter(run => run.argv.includes('serve'))).toHaveLength(3)
})

test('a tree a tool call reaches is registered with the page server once', async ($, on) => {
  const w = world(on)
  await start($)
  w.state.read = readDoc({ tree: '/work/other' })
  await $.tool.call({ tool: 'mcp__loft__yard_read' })
  await $.tool.call({ tool: 'mcp__loft__yard_read' })
  await w.clock.settle()
  const other = w.state.runs.filter(run => run.argv.includes('serve') && flagOf(run.argv, '--tree') === '/work/other')
  expect(other).toHaveLength(1)
  expect(other[0]!.cwd).toBe('/work/other')
})

test('the token is read from the home directory', async ($, on) => {
  const w = world(on)
  await start($)
  await $.tool.call({ tool: 'Bash', command: 'ls' })
  await w.clock.settle()
  expect(w.state.posts[0]!.headers.Authorization).toBe('Bearer token-1')
  expect(HOME).toBe('/home/reader')
})

test('the context is given again after /resume and /branch, and a session a new process starts names no session it continues', async ($, on) => {
  const w = world(on)
  await start($)
  const first = w.state.runs.find(run => run.argv.includes('read'))!
  expect(first.argv).not.toContain('--continues')
  for (const source of ['resume', 'fork'] as const) {
    const again = await $.classic.SessionStart({ source })
    expect((again.additionalContext ?? []).join('\n')).toContain('Yard Log of `feat/shelf`')
  }
})

test("the main agent's call names no agent, and an input at fault runs no command", async ($, on) => {
  const w = world(on)
  await start($)
  await $.tool.call({ tool: 'mcp__loft__yard_read' })
  expect(w.state.runs.at(-1)!.argv).not.toContain('--agent')
  const before = w.state.runs.length
  await $.tool.call({ tool: 'mcp__loft__yard_wait' })
  await $.tool.call({ tool: 'mcp__loft__yard_close', force: true })
  expect(w.state.runs.length).toBe(before)
})

test('with a loftCommand that cannot be run, the agent asks and plans in Claude Code alone', async ($, on) => {
  const w = world(on, { protocol: 'unrunnable' })
  await start($)
  await $.tool.call({ tool: 'AskUserQuestion', questions: [{ question: 'Which?', header: 'Pick', multiSelect: false, options: [{ label: 'A', description: '' }, { label: 'B', description: '' }] }] })
  await $.tool.call({ tool: 'ExitPlanMode', plan: '# P', planFilePath: '/p/plan.md' } as never)
  expect(w.state.calls.map(call => call.tool)).toEqual(['AskUserQuestion', 'ExitPlanMode'])
  expect(w.state.runs.filter(run => !run.argv.includes('protocol'))).toEqual([])
})

test('a loft of an older protocol than the plugin speaks leaves Yard Client off and names loft to update', async ($, on) => {
  const w = world(on, { protocol: 0 })
  await start($)
  expect(w.state.tools).toEqual([])
  expect(w.state.toasts[0]).toMatch(/which speaks the yard protocol 0\. To use Yard Client, bring loft up to date/)
})

test('with Yard Client off, the notice asks loftCommand alone for its version', { options: { loftCommand: 'pnpm exec loft' } }, async ($, on) => {
  const w = world(on, { protocol: 'none' })
  await start($)
  expect(w.state.runs.map(run => run.argv)).toEqual([
    ['pnpm', 'exec', 'loft', 'yard', 'protocol', '--format', 'json'],
    ['pnpm', 'exec', 'loft', '--version'],
  ])
  for (const run of w.state.runs) expect(run.env).toEqual({ LOFT_TELEMETRY: 'off', LOFT_UPDATE_CHECK: 'off' })
})

test('the mod registers its tree with the baseBranch setting, against which the page checks a merge', { options: { baseBranch: 'main' } }, async ($, on) => {
  const w = world(on)
  await start($)
  const serve = w.state.runs.find(run => run.argv.includes('serve'))!
  expect(flagOf(serve.argv, '--base')).toBe('main')
  expect(flagOf(serve.argv, '--tree')).toBe(TREE)
})
