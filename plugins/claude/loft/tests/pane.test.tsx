import { expect, mock, test } from 'claude-code/testing'

import { TONES } from '../hooks/ui'
import { FEATURE, ROOT, type Variant, documents } from './fixtures'

type Body = Extract<Parameters<typeof test>[1], (...args: never[]) => unknown>
type Engine = Parameters<Body>[0]
type On = Parameters<Body>[1]

const SURFACES = ['terminal', 'desktop'] as const

const PANE = {
  title: 'Loft',
  isFocused: false,
  bodyColumns: 72,
  placement: 'dock',
  scroll: { offset: 0, bodyRows: 60 },
  view: {},
} as const

const out = (json: unknown, exitCode = 0) => ({
  value: { exitCode, stdout: JSON.stringify(json), stderr: '', isStdoutTruncated: false, isStderrTruncated: false },
})

/** The world beneath the plugin: `loft`, the file system and the host, answered from memory. */
function world(on: On, options: { root?: string | null; variant?: Variant } = {}) {
  const clock = mock.clock(on)
  const state = {
    variant: options.variant ?? {},
    runs: [] as { argv: readonly string[]; cwd?: string; env?: Record<string, string> }[],
    statuses: [] as (string | undefined)[],
    fail: undefined as undefined | 'unrunnable' | 'old' | 'stops',
    pending: undefined as undefined | Promise<void>,
    opened: [] as string[],
    files: [] as string[],
  }
  const root = options.root === undefined ? ROOT : options.root
  on('fs.exists', ($, e) => ({ value: root !== null && e.path === `${root}/spec/loftspec.yaml` }))
  on('ui.open', ($, e) => {
    state.opened.push(e.id)
    return { value: { isPlaced: true } }
  })
  on('fs.read', ($, e) => {
    state.files.push(`read ${e.path}`)
    return { deny: 'the pane reads no file' }
  })
  on('fs.write', ($, e) => {
    state.files.push(`write ${e.path}`)
    return { deny: 'the pane writes no file' }
  })
  on('command.register', ($, e) => ({ value: { command: e.name } }))
  on('session.start', ($, e) => ({ cwd: e.cwd }))
  on('turn.complete', () => ({ text: '' }))
  on('ui.status', ($, e) => {
    state.statuses.push(e.text)
    return { value: undefined }
  })
  // Yard Client's check of `loft yard protocol` meets a loft with no yard commands, and stays off:
  // the pane works as it does with a loft that has them.
  on('ui.toast', () => ({ value: undefined }))
  on('process.run', async ($, e) => {
    if (e.argv.includes('yard') && state.fail === 'unrunnable') return { deny: 'loft is not on PATH' }
    if (e.argv.includes('yard') || e.argv.includes('--version')) {
      return { value: { exitCode: 2, stdout: '', stderr: "error: unrecognized subcommand 'yard'", isStdoutTruncated: false, isStderrTruncated: false } }
    }
    state.runs.push({ argv: e.argv, cwd: e.init?.cwd, env: e.init?.env })
    if (state.pending) await state.pending
    if (state.fail === 'unrunnable') return { deny: 'loft is not on PATH' }
    const sub = e.argv.find(word => ['status', 'fmt', 'check', 'reconcile'].includes(word))
    if (state.fail === 'old' && sub === 'status') {
      return { value: { exitCode: 2, stdout: '', stderr: "error: unrecognized subcommand 'status'", isStdoutTruncated: false, isStderrTruncated: false } }
    }
    if (state.fail === 'stops' && sub === 'status') {
      return out({ schema: 'loft-status/1', outcome: 'error', errors: [{ kind: 'read', message: 'a tab in the indentation', path: FEATURE, line: 7, sites: [] }] }, 1)
    }
    const d = documents(state.variant)
    if (sub === 'status') return out(d.status)
    if (sub === 'fmt') return out(d.fmt, d.fmt.outcome === 'canonical' ? 0 : 1)
    if (sub === 'check') return out(d.check, d.check.outcome === 'passed' ? 0 : 1)
    return out(d.reconcile)
  })
  on('tool.call', () => ({ result: 'ok', text: 'ok' }))
  return { clock, state }
}

const start = ($: Engine, cwd = `${ROOT}/src`) => $.session.start({ cwd, surface: 'terminal', isInteractive: true })

const command = ($: Engine, args = '') =>
  $.command.run({ command: 'loft-status', args, origin: { kind: 'composer' }, presentation: { isFullscreen: true, columns: 160 } })

const mount = ($: Engine, surface: (typeof SURFACES)[number]) =>
  $.ui.mount({ plugin: 'loft', surface, component: 'Pane', requestId: 'loft-status', viewport: { columns: 76, rows: 64 }, props: PANE })

const statusRuns = (runs: { argv: readonly string[] }[]) => runs.filter(run => run.argv.includes('status'))

test('a session below the specification follows it and reads it with loft --format json, quietly', async ($, on) => {
  const { state } = world(on)
  await start($)
  const subs = state.runs.map(run => run.argv.slice(1).join(' '))
  expect(subs).toEqual([
    'status --base develop --format json',
    'fmt --check --format json',
    'check --format json',
    'reconcile --format json',
  ])
  for (const run of state.runs) {
    expect(run.argv[0]).toBe('loft')
    expect(run.cwd).toBe(ROOT)
    expect(run.env).toEqual({ LOFT_TELEMETRY: 'off', LOFT_UPDATE_CHECK: 'off' })
  }
})

test('a loftCommand of several words runs its first word with the others first', { options: { loftCommand: 'pnpm exec loft' } }, async ($, on) => {
  const { state } = world(on)
  await start($)
  expect(state.runs[0]!.argv.slice(0, 4)).toEqual(['pnpm', 'exec', 'loft', 'status'])
})

test('the baseBranch setting is the base by default', { options: { baseBranch: 'main' } }, async ($, on) => {
  const { state } = world(on)
  await start($)
  expect(state.runs[0]!.argv).toContain('main')
})

test('without a specification the pane says so, runs no loft command and clears the status line', async ($, on) => {
  const { state } = world(on, { root: null })
  await start($)
  expect(state.runs).toEqual([])
  expect(state.statuses.at(-1)).toBeUndefined()
  const answer = await command($)
  expect(answer?.text).toMatch(/found no Loft specification/)
  for (const surface of SURFACES) {
    const ui = await mount($, surface)
    expect(await ui.find({ type: 'Text', text: /No Loft specification here/ })).toBeDefined()
    await ui.unmount()
  }
})

test('/loft-status opens the pane', async ($, on) => {
  const { state } = world(on)
  await start($)
  state.opened.length = 0
  await command($)
  expect(state.opened).toEqual(['loft-status'])
})

test('/loft-status names the base it follows, takes a revision for the session and auto returns to the setting', async ($, on) => {
  const { state } = world(on)
  await start($)
  expect((await command($))?.text).toBe('The Loft status pane compares the tree with `develop`.')
  expect((await command($, 'release/2'))?.text).toBe('The Loft status pane compares the tree with `release/2`.')
  expect(statusRuns(state.runs).at(-1)!.argv).toContain('release/2')
  expect((await command($, 'auto'))?.text).toBe('The Loft status pane compares the tree with `develop`.')
  expect(statusRuns(state.runs).at(-1)!.argv).toContain('develop')
})

test('the pane shows each check with a glyph and a word, then the next step', async ($, on) => {
  world(on)
  await start($)
  for (const surface of SURFACES) {
    const ui = await mount($, surface)
    expect(await ui.find({ type: 'Text', text: /^✓ Format\s+done · canonical$/ })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /^○ Check\s+to do · 0 errors · 1 warning$/ })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /^✓ Spec committed\s+done · clean$/ })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /^○ Trace\s+to do · 1\/2 criteria reviewed$/ })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /^○ Lock\s+to do · unpublished changes since publish 3$/ })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: 'Next: fix what `loft check` reports' })).toBeDefined()
    await ui.unmount()
  }
})

test('the pane lists the nodes added, changed and removed with their counts, labels and names', async ($, on) => {
  world(on)
  await start($)
  for (const surface of SURFACES) {
    const ui = await mount($, surface)
    expect(await ui.find({ type: 'Text', text: 'This branch  +1 ~2 −1' })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /\+ added\s+new\s+An added item shows in the basket\./ })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /~ changed\s+AC-4\s+An item added twice counts two\./ })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /− removed\s+AC-9\s+An empty basket says so\./ })).toBeDefined()
    await ui.unmount()
  }
})

test('a list longer than the pane ends with a line that counts what it leaves out', async ($, on) => {
  const many = Array.from({ length: 80 }, (_, n) => ({
    key: `k-${n}`, label: null, kind: 'requirement', path: FEATURE, line: n + 1, column: 10, name: `Requirement ${n}`,
  }))
  world(on, { variant: { added: many } })
  await start($)
  const ui = await mount($, 'terminal')
  expect(await ui.find({ type: 'Text', text: /^ {2}… \d+ more$/ })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /Requirement 79/ })).toBeUndefined()
  await ui.unmount()
})

test('the criteria to verify mark the reviewed, the current one and the message of an open one', async ($, on) => {
  world(on)
  await start($)
  for (const surface of SURFACES) {
    const ui = await mount($, surface)
    expect(await ui.find({ type: 'Text', text: 'Criteria to verify  1/2 reviewed' })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /✓ reviewed AC-4\s+An item added twice counts two\.$/ })).toBeDefined()
    expect(
      await ui.find({ type: 'Text', text: /▶ current\s+new\s+An added item shows in the basket\. — no link names this node/ }),
    ).toBeDefined()
    await ui.unmount()
  }
})

test('a write under spec shows that file and its nodes as the one being worked on, and starts a read', async ($, on) => {
  const { clock, state } = world(on)
  await start($)
  const before = statusRuns(state.runs).length
  await $.tool.call({ tool: 'Write', file_path: `${ROOT}/${FEATURE}`, content: 'x' })
  await clock.advance(400)
  expect(statusRuns(state.runs).length).toBeGreaterThan(before)
  const ui = await mount($, 'terminal')
  expect(await ui.find({ type: 'Text', text: `  ▸ ${FEATURE}` })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /^ {4}~ AC-4\s+An item added twice counts two\./ })).toBeDefined()
  await ui.unmount()
})

test('a write of the trace, a shell command that runs loft or git, and the loft-spec skill each start a read', async ($, on) => {
  const { clock, state } = world(on)
  await start($)
  const reads = () => statusRuns(state.runs).length
  for (const call of [
    { tool: 'Edit', file_path: `${ROOT}/loft.trace.yaml`, old_string: 'a', new_string: 'b' },
    { tool: 'Bash', command: 'git commit -m spec' },
    { tool: 'Skill', skill: 'loft:loft-spec' },
  ] as const) {
    const before = reads()
    await $.tool.call(call as never)
    await clock.advance(400)
    expect(reads()).toBe(before + 1)
  }
  const before = reads()
  await $.tool.call({ tool: 'Bash', command: 'cargo build' })
  await clock.advance(400)
  expect(reads()).toBe(before)
})

test('the end of a turn starts a read, and so does every tenth second', async ($, on) => {
  const { clock, state } = world(on)
  await start($)
  const reads = () => statusRuns(state.runs).length
  let before = reads()
  await $.turn.complete({ answer: '', durationMs: 1, isAborted: false, turnId: 't1', reason: 'answer' })
  await clock.advance(400)
  expect(reads()).toBe(before + 1)
  before = reads()
  await clock.advance(10000)
  expect(reads()).toBe(before + 1)
})

test('no read starts while another runs, and one asked for meanwhile runs after it', async ($, on) => {
  const { clock, state } = world(on)
  await start($)
  let release!: () => void
  state.pending = new Promise(resolve => (release = resolve))
  const reads = () => statusRuns(state.runs).length
  const before = reads()
  await $.turn.complete({ answer: '', durationMs: 1, isAborted: false, turnId: 't1', reason: 'answer' })
  await clock.advance(400)
  expect(reads()).toBe(before + 1)
  await $.turn.complete({ answer: '', durationMs: 1, isAborted: false, turnId: 't2', reason: 'answer' })
  await clock.advance(400)
  expect(reads()).toBe(before + 1)
  state.pending = undefined
  release()
  await clock.advance(400)
  expect(reads()).toBe(before + 2)
})

test('the status line sums up the branch', async ($, on) => {
  const { state } = world(on)
  await start($)
  expect(state.statuses.at(-1)).toBe('feature/basket · +1 ~2 −1 · check 0e/1w · trace 1/2')
})

test('a loftCommand that cannot be run shows a notice naming the setting and no check', async ($, on) => {
  const { state } = world(on)
  state.fail = 'unrunnable'
  await start($)
  const ui = await mount($, 'terminal')
  expect(await ui.find({ type: 'Text', text: /`loft` cannot be run .*loftCommand/ })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /Format/ })).toBeUndefined()
  await ui.unmount()
})

test('a loft without a status command is reported as too old for the pane', async ($, on) => {
  const { state } = world(on)
  state.fail = 'old'
  await start($)
  const ui = await mount($, 'terminal')
  expect(await ui.find({ type: 'Text', text: /has no `status` command: it is too old/ })).toBeDefined()
  await ui.unmount()
})

test('a command that stops shows its error with its place and keeps the last complete read', async ($, on) => {
  const { clock, state } = world(on)
  await start($)
  state.fail = 'stops'
  await $.turn.complete({ answer: '', durationMs: 1, isAborted: false, turnId: 't1', reason: 'answer' })
  await clock.advance(400)
  const ui = await mount($, 'terminal')
  expect(await ui.find({ type: 'Text', text: `✗ status: a tab in the indentation (${FEATURE}:7)` })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /^✓ Format\s+done · canonical$/ })).toBeDefined()
  await ui.unmount()
})

test('every text the pane draws takes a tone of the table, and none is dimmed', async ($, on) => {
  const { clock, state } = world(on)
  await start($)
  state.fail = 'stops'
  await $.tool.call({ tool: 'Write', file_path: `${ROOT}/${FEATURE}`, content: 'x' })
  await clock.advance(400)
  const colors = new Set(Object.values(TONES).map(tone => tone.color))
  for (const surface of SURFACES) {
    const ui = await mount($, surface)
    const texts = await ui.findAll({ type: 'Text' })
    expect(texts.length).toBeGreaterThan(10)
    for (const text of texts) {
      expect(colors.has(text.props.color as never)).toBe(true)
      expect(text.props.dimColor).toBeUndefined()
    }
    await ui.unmount()
  }
})

test('the pane runs no command but loft, each with --format json, and reads and writes no file', async ($, on) => {
  const { clock, state } = world(on)
  await start($)
  await $.tool.call({ tool: 'Write', file_path: `${ROOT}/${FEATURE}`, content: 'x' })
  await $.tool.call({ tool: 'Bash', command: 'git commit -m spec' })
  await command($, 'main')
  await $.turn.complete({ answer: '', durationMs: 1, isAborted: false, turnId: 't1', reason: 'answer' })
  await clock.advance(10400)
  const ui = await mount($, 'terminal')
  await ui.unmount()
  expect(state.runs.length).toBeGreaterThan(8)
  expect(state.runs.every(run => run.argv[0] === 'loft')).toBe(true)
  expect(state.runs.every(run => run.argv.slice(-2).join(' ') === '--format json')).toBe(true)
  expect(state.files).toEqual([])
})
