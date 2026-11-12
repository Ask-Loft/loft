// The Yard pane's Gantt chart, as a pure model of the view (the Feature "See Yard Log in the pane
// as on the Yard page"): its rows, its waits, its axis, its SVG image and its terminal bars.

import { expect, test } from 'claude-code/testing'

import type { PaneBlock, PaneStage, PaneView } from '../types'
import { altOf, barsOf, ganttOf, MAX_SVG, svgOf, ticksOf } from '../hooks/adapter/pane/gantt'
import { PALETTE } from '../hooks/adapter/pane/theme'

const at = (hhmm: string) => `2026-10-22T${hhmm}:00Z`
const ms = (hhmm: string) => Date.parse(at(hhmm))
const NOW = ms('09:40')

function task(id: string, body: Record<string, unknown>, times?: Record<string, string>): PaneBlock {
  return { id, key: `k-${id}`, kind: 'task', version: { body: { title: id, ...body } }, ...(times ? { times } : {}) }
}

function stage(id: string, status: string, times: Record<string, string> | undefined, blocks: PaneBlock[] = []): PaneStage {
  return { id, key: `k-${id}`, kind: 'stage', version: { body: { title: `Stage ${id}`, status } }, ...(times ? { times } : {}), blocks }
}

const VIEW: PaneView = {
  tree: '/t',
  repository: '/t',
  branch: 'b',
  title: 'b',
  log: 'l',
  revision: 1,
  page: null,
  stages: [
    stage('shape', 'done', { started: at('08:00'), ended: at('08:30') }),
    stage('build', 'active', { started: at('08:40') }, [
      task('store', { state: 'done' }, { started: at('08:45'), ended: at('09:10') }),
      task('mend', { state: 'active', type: 'fix' }, { started: at('09:15') }),
      task('later', { state: 'open' }),
      { id: 'q', key: 'k-q', kind: 'question', version: { body: {} } },
    ]),
    stage('review', 'planned', undefined),
  ],
  sent: [],
  asked: [],
  submissions: [
    { key: 's-1', blocks: ['k-q'], sent: at('09:05'), settled: at('09:12'), isOpen: false },
    { key: 's-2', blocks: ['k-q'], sent: at('09:30'), settled: null, isOpen: true },
  ],
}

test('each stage that started is a row from when it started to when it ended, or to now', () => {
  const rows = ganttOf(VIEW, NOW).rows.filter(row => row.kind === 'stage')
  expect(rows.map(row => [row.key, row.from, row.to, row.tone])).toEqual([
    ['k-shape', ms('08:00'), ms('08:30'), 'done'],
    ['k-build', ms('08:40'), NOW, 'active'],
  ])
})

test('each task whose work started is a row beneath its stage, to when it ended or now, a fix in the fix tone', () => {
  const rows = ganttOf(VIEW, NOW).rows
  expect(rows.map(row => row.key)).toEqual(['k-shape', 'k-build', 'k-store', 'k-mend'])
  expect(rows.slice(2).map(row => [row.kind, row.stage, row.from, row.to, row.tone])).toEqual([
    ['task', 'k-build', ms('08:45'), ms('09:10'), 'task'],
    ['task', 'k-build', ms('09:15'), NOW, 'fix'],
  ])
})

test("each wait lies on its stage's bar, from when its submission was sent to when it was settled, or to now", () => {
  const build = ganttOf(VIEW, NOW).rows.find(row => row.key === 'k-build')!
  expect(build.waits).toEqual([
    { from: ms('09:05'), to: ms('09:12') },
    { from: ms('09:30'), to: NOW },
  ])
})

test('the axis marks whole steps of time between the start and the end of the chart', () => {
  const chart = ganttOf(VIEW, NOW)
  expect([chart.start, chart.end]).toEqual([ms('08:00'), NOW])
  const ticks = ticksOf(chart, 6)
  expect(ticks.length).toBeGreaterThan(1)
  expect(ticks.length).toBeLessThanOrEqual(7)
  expect(ticks.every(tick => tick >= chart.start && tick <= chart.end)).toBe(true)
  expect(ticks).toEqual([...ticks].sort((a, b) => a - b))
  expect(ticks.every(tick => tick % (10 * 60_000) === 0)).toBe(true)
})

test('the text alternative names each stage and how long it ran', () => {
  const alt = altOf(ganttOf(VIEW, NOW))
  expect(alt).toContain('Stage shape ran 30 min')
  expect(alt).toContain('Stage build has run 1 h, and goes on')
  expect(alt).not.toContain('Stage review')
})

test('the SVG image draws each row in its tone, escapes its titles and stays within the size a surface draws', () => {
  const svg = svgOf(ganttOf(VIEW, NOW), PALETTE)
  expect(svg.startsWith('<svg')).toBe(true)
  expect(svg).toContain('@media (prefers-color-scheme: dark)')
  expect(svg).toContain(`fill="${PALETTE.done}"`)
  expect(svg).toContain(`fill="${PALETTE.fix}"`)
  expect(svg).toContain(`fill="${PALETTE.wait}"`)
  const many: PaneView = {
    ...VIEW,
    stages: [stage('<&>', 'active', { started: at('08:00') }, Array.from({ length: 3000 }, (_, n) => task(`t${n}`, { state: 'done' }, { started: at('08:01'), ended: at('08:02') })))],
  }
  const big = svgOf(ganttOf(many, NOW), PALETTE)
  expect(big.length).toBeLessThanOrEqual(MAX_SVG)
  expect(big).toContain('Stage &lt;&amp;&gt;')
  expect(big).not.toContain('Stage <&>')
})

test("on the terminal each row's bar starts and ends at the columns of its times along the chart", () => {
  const chart = ganttOf(VIEW, NOW)
  const width = 50
  const column = (time: number) => Math.round(((time - chart.start) / (chart.end - chart.start)) * (width - 1))
  for (const row of barsOf(chart, width)) {
    const drawn = row.runs.map(run => run.text).join('')
    expect(drawn).toHaveLength(width)
    const first = drawn.search(/\S/)
    const last = drawn.length - 1 - [...drawn].reverse().join('').search(/\S/)
    const source = chart.rows.find(each => each.key === row.key)!
    expect([first, last]).toEqual([column(source.from), column(source.to)])
  }
})
