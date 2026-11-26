import { expect, test } from 'claude-code/testing'

import { HOME, start, world, yardRuns } from './yard-world'

const CARGO_LOFT = `${HOME}/.cargo/bin/loft`
const CARGO_LOFT_EXE = `${HOME}/.cargo/bin/loft.exe`

test('where the bare loft cannot be run, the loft in ~/.cargo/bin turns Yard Client on and runs every later command', async ($, on) => {
  const w = world(on, { runnable: [CARGO_LOFT] })
  await start($)
  expect(w.state.tools.map(tool => tool.name)).toContain('yard_read')
  expect(w.state.toasts).toEqual([])
  await $.tool.call({ tool: 'mcp__loft__yard_read' })
  await w.clock.advance(5000)
  const yards = w.state.runs.filter(run => run.argv.includes('yard'))
  expect(yards[0]?.argv[0]).toBe('loft')
  const after = yards.slice(1).map(run => run.argv[0])
  expect(after.length).toBeGreaterThan(1)
  expect(new Set(after)).toEqual(new Set([CARGO_LOFT]))
})

test('where only loft.exe lies in ~/.cargo/bin, as on Windows, the mod runs it', async ($, on) => {
  const w = world(on, { runnable: [CARGO_LOFT_EXE] })
  await start($)
  expect(w.state.tools.map(tool => tool.name)).toContain('yard_read')
  expect(yardRuns(w)).toContain('protocol --format json')
  expect(w.state.runs.at(-1)?.argv[0]).toBe(CARGO_LOFT_EXE)
})

test('a loftCommand the person set that cannot be run is not looked for in ~/.cargo/bin', { options: { loftCommand: 'pnpm exec loft' } }, async ($, on) => {
  const w = world(on, { runnable: [CARGO_LOFT] })
  await start($)
  expect(w.state.tools).toEqual([])
  expect(w.state.runs.map(run => run.argv[0])).not.toContain(CARGO_LOFT)
  expect(w.state.toasts).toHaveLength(1)
  expect(w.state.toasts[0]).toMatch(/Yard Client is off: `pnpm exec loft` cannot be run/)
  expect(w.state.toasts[0]).not.toContain('.cargo')
})

test('where no loft can be run, the notice names each command the mod tried', async ($, on) => {
  const w = world(on, { runnable: [] })
  await start($)
  expect(w.state.tools).toEqual([])
  expect(w.state.toasts).toHaveLength(1)
  const notice = w.state.toasts[0] ?? ''
  for (const tried of ['`loft`', `\`${CARGO_LOFT}\``, `\`${CARGO_LOFT_EXE}\``]) expect(notice).toContain(tried)
  expect(notice).toMatch(/^Yard Client is off: `loft` cannot be run \([^)]* loft: no such file\)\./)
  expect(notice).not.toContain('loft.exe: no such file')
  expect(notice).toMatch(/loftCommand setting/)
})
