import { expect, test } from 'claude-code/testing'

import { PLUGIN_VERSION } from '../hooks/adapter'
import { deferred, flagOf, HOME, readDoc, SESSION, spawn, start, stage, TREE, world, yardRuns, type On } from './yard-world'

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
  expect(w.state.tools).toEqual([])
  expect(w.state.toasts).toHaveLength(1)
  expect(w.state.toasts[0]).toMatch(new RegExp(`Yard Client is off: the loft plugin ${PLUGIN_VERSION.replaceAll('.', '\\.')} speaks the yard protocol 1–5, and \`loft\` runs loft .*, which has no yard commands\\. To use Yard Client, bring loft up to date, or set the loft plugin's loftCommand setting\\.`))
  const denied = await $.tool.call({ tool: 'mcp__loft__yard_read' })
  expect(denied.deny ?? denied.text).toMatch(/Yard Client is off/)
})

test('a loft of a newer protocol than the plugin speaks leaves Yard Client off and names the plugin to update', async ($, on) => {
  const w = world(on, { protocol: 6 })
  await start($)
  expect(w.state.tools).toEqual([])
  expect(w.state.toasts[0]).toMatch(/runs loft 2\.2\.1, which speaks the yard protocol 6\. To use Yard Client, bring the loft plugin up to date/)
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

test("the mod reads no briefing at the session's start or at a prompt: the plugin's command hooks give the context", async ($, on) => {
  const w = world(on)
  await start($)
  await $.turn.start({ text: 'go', turnId: 't1' })
  await w.clock.advance(1)
  expect(w.state.runs.filter(run => run.argv.includes('read'))).toEqual([])
})

test("after /clear the new session's first turn records once the session it continues, before any other command", async ($, on) => {
  const w = world(on)
  await start($)
  await $.session.end({ reason: 'clear', sessionId: SESSION } as never)
  w.state.session = 'session-2'
  const before = yardRuns(w).length
  await $.turn.start({ text: 'go', turnId: 't1' })
  await $.tool.call({ tool: 'mcp__loft__yard_read' })
  await w.clock.advance(5000)
  const after = yardRuns(w).slice(before)
  expect(after[0]).toBe('continue --session session-2 --from session-1 --format json')
  expect(after.filter(run => run.startsWith('continue'))).toHaveLength(1)
  for (const run of after.slice(1)) expect(run).not.toContain(SESSION)
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

test('each tool runs its command in the session directory with the page server port, and a write with the baseBranch setting as its default base', { options: { baseBranch: 'main', yardPort: 7500 } }, async ($, on) => {
  const w = world(on)
  await start($)
  w.state.cwd = `${TREE}/src`
  await $.tool.call({ tool: 'mcp__loft__yard_read' })
  const read = w.state.runs.at(-1)!
  expect(read.cwd).toBe(`${TREE}/src`)
  expect(flagOf(read.argv, '--port')).toBe('7500')
  expect(read.argv).not.toContain('--base')
  await $.tool.call({ tool: 'mcp__loft__yard_write', batch: [{ id: 'shape', kind: 'stage', title: 'Shape', status: 'active' }] })
  const write = w.state.runs.at(-1)!
  expect(flagOf(write.argv, '--default-base')).toBe('main')
  expect(write.argv).not.toContain('--base')
  expect(flagOf(write.argv, '--port')).toBe('7500')
  for (const run of w.state.runs.filter(each => !each.argv.includes('write'))) expect(run.argv).not.toContain('--default-base')
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

test('yard_read gives each loft report only when the agent asks for reports', async ($, on) => {
  const w = world(on)
  await start($)
  const before = w.state.runs.length
  await $.tool.call({ tool: 'mcp__loft__yard_read' })
  await $.tool.call({ tool: 'mcp__loft__yard_read', since: 3, reports: true })
  const [plain, asked] = w.state.runs.slice(before).filter(run => run.argv.includes('read') && run.argv.includes('--mark-read'))
  expect(plain?.argv).not.toContain('--reports')
  expect(asked?.argv).toContain('--reports')
  expect(flagOf(asked?.argv ?? [], '--since')).toBe('3')
})

test('output that does not read as a document is named by its size, not by the exit code', async ($, on) => {
  const w = world(on)
  await start($)
  w.state.printed.read = { stdout: '{"schema": "loft-yard-read/1", "outc' }
  const failed = await $.tool.call({ tool: 'mcp__loft__yard_read' })
  const error = String(failed.deny ?? failed.text)
  expect(error).toContain('loft printed 36 bytes on standard output that could not be read as a document')
  expect(error).not.toMatch(/^exit \d+$/)
})

test('output Claude Code cut short says so, after the bytes it kept', async ($, on) => {
  const w = world(on)
  await start($)
  w.state.printed.read = { stdout: '{"schema": "loft-yard-read/1", "stages": [', truncated: true }
  const failed = await $.tool.call({ tool: 'mcp__loft__yard_read' })
  expect(String(failed.deny ?? failed.text)).toContain('cut short by Claude Code after 42 bytes')
})

test('output cut short is never read as a document, even where the bytes kept would read as one', async ($, on) => {
  const w = world(on)
  await start($)
  w.state.printed.read = { stdout: JSON.stringify(w.state.read), truncated: true }
  const failed = await $.tool.call({ tool: 'mcp__loft__yard_read' })
  expect(failed.deny).toMatch(/could not be read as a document, cut short by Claude Code after \d+ bytes/)
})

test('a subagent calls a tool in its own name: its id and its type', async ($, on) => {
  const w = world(on)
  await start($)
  await spawn($, w, 'a9', 'Explore')
  await $.tool.call({ tool: 'mcp__loft__yard_read', agentId: 'a9' })
  const run = w.state.runs.at(-1)!
  expect(flagOf(run.argv, '--agent')).toBe('a9')
  expect(flagOf(run.argv, '--agent-type')).toBe('Explore')
  w.state.agents = [{ id: 'b2', type: 'Plan' }]
  await $.tool.call({ tool: 'mcp__loft__yard_read', agentId: 'b2' })
  expect(flagOf(w.state.runs.at(-1)!.argv, '--agent-type')).toBe('Plan')
})

test('where no page server answers, the mod runs loft yard serve for the session and its tree, and one that serves is not run again', async ($, on) => {
  const w = world(on, { serve: 'serving' })
  await start($)
  await w.clock.advance(1)
  const serves = () => w.state.runs.filter(run => run.argv.includes('serve'))
  expect(serves()).toHaveLength(1)
  expect(serves()[0]!.argv).toEqual(['loft', 'yard', 'serve', '--session', SESSION, '--tree', TREE, '--port', '7411', '--format', 'json'])
  await w.clock.advance(15000)
  expect(serves()).toHaveLength(1)
  w.state.serverStops.resolve()
  await w.clock.advance(5000)
  expect(serves().length).toBeGreaterThan(1)
})

test('the mod passes each address of the yardListen setting to the loft yard serve it starts', { options: { yardListen: '100.64.0.1, 192.168.1.5' } }, async ($, on) => {
  const w = world(on, { serve: 'serving', protocol: 4 })
  await start($)
  await w.clock.advance(1)
  const serve = w.state.runs.find(run => run.argv.includes('serve'))!
  expect(serve.argv).toEqual(['loft', 'yard', 'serve', '--session', SESSION, '--tree', TREE, '--listen', '100.64.0.1', '--listen', '192.168.1.5', '--host', 'claude-code', '--port', '7411', '--format', 'json'])
  await $.tool.call({ tool: 'mcp__loft__yard_read' })
  expect(w.state.runs.at(-1)!.argv).not.toContain('--listen')
})

test('the mod names once each entry of the yardListen setting that is not an IP address or is the unspecified IPv6 address, and passes the other addresses', { options: { yardListen: '100.64.0.1, machine.example.ts.net 999.1.1.1 ::' } }, async ($, on) => {
  const w = world(on, { serve: 'running', protocol: 4 })
  await start($)
  await w.clock.advance(10000)
  expect(w.state.toasts).toEqual([
    "The loft plugin's yardListen setting names `machine.example.ts.net`, `999.1.1.1`, `::`, which the page server does not listen on: name IP addresses there, such as the machine's tailnet address, and `0.0.0.0`, not `::`, for every address.",
  ])
  const serve = w.state.runs.find(run => run.argv.includes('serve'))!
  expect(serve.argv.filter((_, at) => serve.argv[at - 1] === '--listen')).toEqual(['100.64.0.1'])
})

test('where no page server answers the beat, the mod runs loft yard serve at every beat', async ($, on) => {
  const w = world(on, { serve: 'running' })
  await start($)
  await w.clock.advance(10000)
  expect(w.state.runs.filter(run => run.argv.includes('serve'))).toHaveLength(3)
})

test('every five seconds the mod tells the page server the session is live, and after the first beat, while one answers, it starts no loft yard serve', async ($, on) => {
  const w = world(on)
  w.state.answersBeat = true
  await start($)
  await w.clock.advance(10000)
  expect(w.state.heard).toEqual([SESSION, SESSION, SESSION])
  expect(w.state.runs.filter(run => run.argv.includes('serve'))).toHaveLength(1)
  w.state.answersBeat = false
  await w.clock.advance(5000)
  expect(w.state.runs.filter(run => run.argv.includes('serve'))).toHaveLength(2)
})

test('at the first beat the mod runs loft yard serve even where a page server answers, so that one on other addresses is replaced', { options: { yardListen: '100.64.0.1' } }, async ($, on) => {
  const w = world(on, { protocol: 4 })
  w.state.answersBeat = true
  await start($)
  await w.clock.advance(1)
  const serves = w.state.runs.filter(run => run.argv.includes('serve'))
  expect(serves).toHaveLength(1)
  expect(flagOf(serves[0]!.argv, '--listen')).toBe('100.64.0.1')
  expect(serves[0]!.argv).not.toContain('--join')
})

test("where the first beat's answer hangs past the next beat, the mod's first loft yard serve still joins no page server", { options: { yardListen: '100.64.0.1' } }, async ($, on) => {
  const w = world(on, { serve: 'running', protocol: 4 })
  const hanging = deferred<void>()
  w.state.hearing = hanging.promise
  await start($)
  w.state.hearing = Promise.resolve()
  await w.clock.advance(5000)
  const serves = w.state.runs.filter(run => run.argv.includes('serve'))
  expect(serves.length).toBeGreaterThan(0)
  expect(serves[0]!.argv).not.toContain('--join')
  hanging.resolve()
  await w.clock.settle()
})

test("a first loft yard serve the mod could not run within 30 seconds of the session's start joins the page server", { options: { yardListen: '100.64.0.1' } }, async ($, on) => {
  const w = world(on, { serve: 'running', protocol: 4 })
  const hanging = deferred<void>()
  w.state.hearing = hanging.promise
  await start($)
  await w.clock.advance(30000)
  w.state.hearing = Promise.resolve()
  hanging.resolve()
  await w.clock.advance(5000)
  const serves = w.state.runs.filter(run => run.argv.includes('serve'))
  expect(serves.length).toBeGreaterThan(0)
  for (const run of serves) expect(run.argv).toContain('--join')
})

test("after the session's first beat each loft yard serve the mod runs joins the page server that runs", async ($, on) => {
  const w = world(on, { serve: 'running', protocol: 4 })
  await start($)
  await w.clock.advance(5000)
  w.state.read = readDoc({ tree: '/work/other' })
  await $.tool.call({ tool: 'mcp__loft__yard_read' })
  await w.clock.settle()
  const serves = w.state.runs.filter(run => run.argv.includes('serve'))
  expect(serves.some(run => flagOf(run.argv, '--tree') === '/work/other')).toBe(true)
  expect(serves.length).toBeGreaterThan(2)
  expect(serves[0]!.argv).not.toContain('--join')
  for (const later of serves.slice(1)) expect(later.argv).toContain('--join')
})

test('to a loft of an older protocol the mod passes neither --listen nor --join, and says once that yardListen waits for a newer loft', { options: { yardListen: '100.64.0.1' } }, async ($, on) => {
  const w = world(on, { serve: 'running', protocol: 3 })
  await start($)
  await w.clock.advance(10000)
  const serves = w.state.runs.filter(run => run.argv.includes('serve'))
  expect(serves.length).toBeGreaterThan(1)
  for (const each of serves) {
    expect(each.argv).not.toContain('--listen')
    expect(each.argv).not.toContain('--join')
  }
  expect(w.state.toasts).toEqual([
    "The loft plugin's yardListen setting waits for a newer loft: loft 2.2.1 speaks the yard protocol 3, so the page opens on this machine alone until loft is brought up to date.",
  ])
})

const registers = (w: ReturnType<typeof world>) => w.state.posts.filter(post => post.url.endsWith('/api/register'))

test("where another session's page server answers, the session's tree is registered with it once, and loft yard serve runs at the first beat alone", async ($, on) => {
  const w = world(on)
  w.state.answersBeat = true
  await start($)
  await w.clock.advance(10000)
  expect(registers(w).map(post => post.events)).toEqual([{ tree: TREE }])
  expect(registers(w)[0]!.url).toBe('http://127.0.0.1:7411/api/register')
  expect(w.state.runs.filter(run => run.argv.includes('serve'))).toHaveLength(1)
})

test('a registration the page server refuses is tried again at the next beat', async ($, on) => {
  const w = world(on)
  w.state.answersBeat = true
  w.state.postStatus = 503
  await start($)
  w.state.postStatus = 200
  await w.clock.advance(5000)
  await w.clock.advance(5000)
  expect(registers(w)).toHaveLength(2)
})

test('the tree the mod\'s own loft yard serve registers is posted once more, to learn its top', async ($, on) => {
  const w = world(on, { serve: 'serving' })
  await start($)
  w.state.answersBeat = true
  await w.clock.advance(10000)
  expect(w.state.heard.length).toBeGreaterThan(1)
  expect(registers(w).map(post => post.events)).toEqual([{ tree: TREE }])
})

test("the session whose mod's child is the page server is heard every five seconds too", async ($, on) => {
  const w = world(on, { serve: 'serving' })
  await start($)
  w.state.answersBeat = true
  await w.clock.advance(10000)
  expect(w.state.heard).toEqual([SESSION, SESSION, SESSION])
  expect(w.state.runs.filter(run => run.argv.includes('serve'))).toHaveLength(1)
})

/** A world whose one page server runs on 7412, as `loft yard serve` reports, while the plugin's port is 7411. */
function runningElsewhere(on: On) {
  const w = world(on, { serve: 'running' })
  w.state.servedPort = 7412
  w.state.reportedPort = 7412
  w.state.answersBeat = true
  return w
}

test('where loft yard serve reports a page server running on another port, the mod tells that page server the session is live', async ($, on) => {
  const w = runningElsewhere(on)
  await start($)
  await w.clock.advance(10000)
  expect(w.state.heardAt.length).toBeGreaterThan(0)
  expect(new Set(w.state.heardAt)).toEqual(new Set(['http://127.0.0.1:7412/api/heard']))
})

test('where loft yard serve reports a page server running on another port, it registers the tree there, and a tree the session moves into is registered with that page server', async ($, on) => {
  const w = runningElsewhere(on)
  await start($)
  await w.clock.advance(5000)
  expect(flagOf(w.state.runs.find(run => run.argv.includes('serve'))!.argv, '--tree')).toBe(TREE)
  w.state.cwd = '/work/other'
  await w.clock.advance(5000)
  expect(registers(w).map(post => post.url)).toEqual(['http://127.0.0.1:7412/api/register', 'http://127.0.0.1:7412/api/register'])
  expect(registers(w).map(post => post.events)).toEqual([{ tree: TREE }, { tree: '/work/other' }])
})

test('while the page server on the reported port answers, the mod starts no more loft yard serve', async ($, on) => {
  const w = runningElsewhere(on)
  await start($)
  await w.clock.advance(15000)
  expect(w.state.runs.filter(run => run.argv.includes('serve'))).toHaveLength(1)
})

test('once no page server answers on the reported port, the next loft yard serve asks for the yardPort setting and the mod follows what it reports', async ($, on) => {
  const w = runningElsewhere(on)
  await start($)
  await w.clock.advance(5000)
  expect(w.state.heardAt.at(-1)).toBe('http://127.0.0.1:7412/api/heard')
  w.state.servedPort = 7411
  w.state.reportedPort = 7411
  w.state.serve = 'serving'
  await w.clock.advance(5000)
  const serves = w.state.runs.filter(run => run.argv.includes('serve'))
  expect(serves).toHaveLength(2)
  expect(flagOf(serves[1]!.argv, '--port')).toBe('7411')
  await w.clock.advance(5000)
  expect(w.state.heardAt.at(-1)).toBe('http://127.0.0.1:7411/api/heard')
  w.state.serverStops.resolve()
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

test('the token is read from the home that loft yard protocol reports', async ($, on) => {
  const w = world(on, { home: '/home/reader/.yard-dogfood' })
  await start($)
  await $.tool.call({ tool: 'Bash', command: 'ls' })
  await w.clock.settle()
  expect(w.state.posts[0]!.headers.Authorization).toBe('Bearer token-1')
})

test('the token is read from the home directory', async ($, on) => {
  const w = world(on)
  await start($)
  await $.tool.call({ tool: 'Bash', command: 'ls' })
  await w.clock.settle()
  expect(w.state.posts[0]!.headers.Authorization).toBe('Bearer token-1')
  expect(HOME).toBe('/home/reader')
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

test('a tool given a tree runs its command with --tree naming it, and a tool given none names no tree', async ($, on) => {
  const w = world(on)
  await start($)
  const calls: [string, Record<string, unknown>][] = [
    ['mcp__loft__yard_read', {}],
    ['mcp__loft__yard_write', { batch: [{ id: 'there', kind: 'stage', title: 'There', status: 'active' }] }],
    ['mcp__loft__yard_withdraw', { submission: 'sub-7' }],
    ['mcp__loft__yard_close', {}],
    ['mcp__loft__yard_published', { branch: 'feat/shelf' }],
  ]
  for (const [tool, input] of calls) {
    await $.tool.call({ tool, ...input, tree: '/work/press' } as never)
    const named = w.state.runs.at(-1)!
    expect(flagOf(named.argv, '--tree')).toBe('/work/press')
    expect(named.cwd).toBe(TREE)
    await $.tool.call({ tool, ...input } as never)
    expect(w.state.runs.at(-1)!.argv).not.toContain('--tree')
  }
  const schemas = w.state.tools.map(each => each.inputSchema as { properties: Record<string, { type: string }> })
  for (const schema of schemas) expect(schema.properties.tree?.type).toBe('string')
})

test('a registration whose answer names no top is posted again at the next beat', async ($, on) => {
  const w = world(on)
  w.state.answersBeat = true
  w.state.registered = {}
  await start($)
  await w.clock.advance(5000)
  expect(registers(w)).toHaveLength(2)
})
