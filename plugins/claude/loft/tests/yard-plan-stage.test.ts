import { expect, test } from 'claude-code/testing'

import { block, readDoc, SESSION, stage, start, world, writesOf, type Engine, type On } from './yard-world'

const PLAN = '# Build the shelf\n\nKeep the books.'
const FILE = '/p/shelf.md'

function configured(on: On) {
  const w = world(on)
  w.state.terminal.ExitPlanMode = () => ({ result: 'approved', text: 'approved' })
  return w
}

const present = ($: Engine, agentId?: string) =>
  $.tool.call({ tool: 'ExitPlanMode', plan: PLAN, planFilePath: FILE, ...(agentId ? { agentId } : {}) } as never)

test('reusing a file attaches a fresh plan to the active stage and preserves the earlier plan', async ($, on) => {
  const w = configured(on)
  const earlier = { ...stage('earlier', 'done', { session: SESSION }), blocks: [{ ...block('plan-shelf', 'plan', { title: 'Earlier plan', markdown: 'Earlier work' }), outcome: { verdict: 'approved' } }] }
  w.state.read = readDoc({ stages: [earlier, stage('current', 'active', { session: SESSION })] })
  const before = JSON.stringify(w.state.read)
  await start($)
  await present($)
  const written = writesOf(w).at(-1)!
  expect(written).toHaveLength(1)
  expect(written[0]).toMatchObject({ kind: 'plan', stage: 'current', markdown: PLAN })
  expect(written[0]!.id).not.toBe('plan-shelf')
  expect(JSON.stringify(w.state.read)).toBe(before)
  const answer = w.state.runs.find(run => run.argv.includes('answer'))!
  expect(JSON.parse(answer.stdin!)).toEqual([{ block: written[0]!.id, verdict: 'approved' }])
})

test('a legacy filename plan is revised only when it belongs to the active stage', async ($, on) => {
  const w = configured(on)
  w.state.read = readDoc({ stages: [{ ...stage('current', 'active', { session: SESSION }), blocks: [block('plan-shelf', 'plan', { title: 'Earlier revision' })] }] })
  await start($)
  await present($)
  expect(writesOf(w).at(-1)).toEqual([{ id: 'plan-shelf', kind: 'plan', stage: 'current', title: 'Build the shelf', markdown: PLAN, planMode: true }])
})

test('repeated file revisions in one active stage keep the same scoped plan id', async ($, on) => {
  const w = configured(on)
  const current = stage('current', 'active', { session: SESSION })
  w.state.read = readDoc({ stages: [current] })
  await start($)
  await present($)
  const first = writesOf(w).at(-1)![0]!
  w.state.read = readDoc({ stages: [{ ...current, blocks: [block(String(first.id), 'plan', { title: 'First revision' })] }] })
  await present($)
  expect(writesOf(w).at(-1)![0]).toMatchObject({ id: first.id, stage: 'current' })
})

test('an earlier file plan does not prevent opening a stage when none is active', async ($, on) => {
  const w = configured(on)
  const earlier = { ...stage('earlier', 'done', { session: SESSION }), blocks: [block('plan-shelf', 'plan', { title: 'Earlier work' })] }
  w.state.read = readDoc({ stages: [earlier] })
  await start($)
  await present($)
  const written = writesOf(w).at(-1)!
  expect(written).toHaveLength(2)
  expect(written[0]).toMatchObject({ kind: 'stage', status: 'active', after: 'earlier' })
  expect(written[1]).toMatchObject({ kind: 'plan', stage: written[0]!.id })
  expect(written[1]!.id).not.toBe('plan-shelf')
})

test('a plan in an automatically opened stage keeps its identity after persistence and a stage rename', async ($, on) => {
  const w = configured(on)
  w.state.read = readDoc()
  await start($)
  await present($)
  const first = writesOf(w).at(-1)!.find(each => each.kind === 'plan')!
  const renamed = { ...stage('renamed', 'active', { session: SESSION }), key: 'stable-stage-key', blocks: [block(String(first.id), 'plan', { title: 'Earlier revision' })] }
  w.state.read = readDoc({ stages: [renamed] })
  await present($)
  expect(writesOf(w).at(-1)).toHaveLength(1)
  expect(writesOf(w).at(-1)![0]).toMatchObject({ id: first.id, stage: 'renamed' })
})

test('the selected stage belongs to the invoking subagent even when another caller holds the same file', async ($, on) => {
  const w = configured(on)
  w.state.read = readDoc({ stages: [
    { ...stage('main', 'active', { session: SESSION }), blocks: [block('plan-shelf', 'plan', { title: 'Main plan' })] },
    stage('review', 'active', { session: SESSION, agent: 'a9', agentType: 'Explore' }),
  ] })
  await start($)
  await $.classic.SubagentStart({ agent_id: 'a9', agent_type: 'Explore' } as never)
  await present($, 'a9')
  const written = writesOf(w).at(-1)![0]!
  expect(written).toMatchObject({ kind: 'plan', stage: 'review' })
  expect(written.id).not.toBe('plan-shelf')
  const command = w.state.runs.filter(run => run.argv.includes('write')).at(-1)!
  expect(command.argv).toContain('a9')
})

test('a plan held by another host cannot override the invoking agent active stage', async ($, on) => {
  const w = configured(on)
  w.state.read = readDoc({ stages: [
    { ...stage('foreign', 'active', { host: 'codex', session: SESSION }), blocks: [block('plan-shelf', 'plan', { title: 'Foreign plan' })] },
    stage('current', 'active', { session: SESSION }),
  ] })
  await start($)
  await present($)
  expect(writesOf(w).at(-1)![0]).toMatchObject({ kind: 'plan', stage: 'current' })
  expect(writesOf(w).at(-1)![0]!.id).not.toBe('plan-shelf')
})
