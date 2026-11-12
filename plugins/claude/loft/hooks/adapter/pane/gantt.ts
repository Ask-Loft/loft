// The Yard pane's Gantt chart as the Yard page draws it, in what the pane can draw: each stage that
// started a row from its start to its end or now, each task whose work started a row beneath it, a
// fix in the fix colour, and each wait on the person on its stage's bar, along a time axis. One SVG
// image where the surface draws SVG, block characters on the terminal (the Feature "See Yard Log in
// the pane as on the Yard page"). Pure.

import type { PaneView } from '../../../types'
import { textOf } from '../../model'
import { clockOf } from './model'
import type { Palette } from './theme'

/** The most an `Svg` element's markup may hold. */
export const MAX_SVG = 131_072

type Span = { from: number; to: number }

/** How a row is coloured: a stage by its status, a task as a task or a fix. */
export type Tone = 'active' | 'done' | 'dropped' | 'task' | 'fix'

export type GanttRow = Span & {
  key: string
  kind: 'stage' | 'task'
  /** The key of the stage the row is or lies in. */
  stage: string
  title: string
  tone: Tone
  /** Whether it has not ended. */
  isOpen: boolean
  /** The waits on the person that lie on a stage's bar. */
  waits: Span[]
}

export type Gantt = { start: number; end: number; rows: GanttRow[] }

const time = (at: string | null | undefined) => (at ? Date.parse(at) : Number.NaN)

/** The rows of the chart, the stages in their order, each followed by its tasks; its start and its end. */
export function ganttOf(view: PaneView, now: number): Gantt {
  const stageOfBlock = new Map(view.stages.flatMap(stage => stage.blocks.map(block => [block.key, stage.key] as const)))
  const waits = new Map<string, Span[]>()
  for (const each of view.submissions) {
    const stage = each.blocks.map(key => stageOfBlock.get(key)).find(key => key !== undefined)
    const from = time(each.sent)
    if (stage === undefined || Number.isNaN(from)) continue
    const settled = time(each.settled)
    waits.set(stage, [...(waits.get(stage) ?? []), { from, to: Number.isNaN(settled) ? Math.max(from, now) : settled }])
  }
  const rows: GanttRow[] = []
  for (const stage of view.stages) {
    const from = time(stage.times?.started)
    if (Number.isNaN(from)) continue
    const ended = time(stage.times?.ended)
    const status = textOf(stage.version.body.status)
    rows.push({
      key: stage.key,
      kind: 'stage',
      stage: stage.key,
      title: textOf(stage.version.body.title) || stage.id,
      from,
      to: Number.isNaN(ended) ? Math.max(from, now) : ended,
      tone: status === 'done' || status === 'dropped' ? status : 'active',
      isOpen: Number.isNaN(ended),
      waits: waits.get(stage.key) ?? [],
    })
    for (const block of stage.blocks) {
      const started = time(block.times?.started)
      if (block.kind !== 'task' || Number.isNaN(started)) continue
      const end = time(block.times?.ended)
      rows.push({
        key: block.key,
        kind: 'task',
        stage: stage.key,
        title: textOf(block.version.body.title) || block.id,
        from: started,
        to: Number.isNaN(end) ? Math.max(started, now) : end,
        tone: block.version.body.type === 'fix' ? 'fix' : 'task',
        isOpen: Number.isNaN(end),
        waits: [],
      })
    }
  }
  const start = Math.min(...rows.map(row => row.from), now)
  const end = Math.max(...rows.map(row => row.to), start + 60_000)
  return { start, end, rows }
}

const STEPS = [1, 2, 5, 10, 15, 30, 60, 120, 240, 480, 1440].map(minutes => minutes * 60_000)

/** The times the axis marks: whole multiples of the smallest step that leaves at most `count` of them. */
export function ticksOf(chart: Gantt, count: number): number[] {
  const span = chart.end - chart.start
  const step = STEPS.find(each => span / each <= count) ?? Math.ceil(span / count / STEPS.at(-1)!) * STEPS.at(-1)!
  const ticks: number[] = []
  for (let tick = Math.ceil(chart.start / step) * step; tick <= chart.end; tick += step) ticks.push(tick)
  return ticks
}

/** A length of time in words, as `1 h 5 min`. */
export function lengthOf(ms: number): string {
  const minutes = Math.round(ms / 60_000)
  if (minutes < 1) return 'under a minute'
  const hours = Math.floor(minutes / 60)
  const rest = minutes % 60
  return [hours ? `${hours} h` : '', rest ? `${rest} min` : ''].filter(Boolean).join(' ')
}

/** The chart in words: each stage and how long it ran, for a reader who cannot see the image. */
export function altOf(chart: Gantt): string {
  const stages = chart.rows.filter(row => row.kind === 'stage')
  const each = stages.map(row => (row.isOpen ? `${row.title} has run ${lengthOf(row.to - row.from)}, and goes on.` : `${row.title} ran ${lengthOf(row.to - row.from)}.`))
  return [`Gantt chart of ${stages.length} ${stages.length === 1 ? 'stage' : 'stages'}.`, ...each].join(' ')
}

/** A row's colour, and a wait's. */
export function toneColour(palette: Palette, tone: Tone | 'wait'): string {
  switch (tone) {
    case 'done':
      return palette.done
    case 'dropped':
      return palette.drop
    case 'task':
      return palette.task
    case 'fix':
      return palette.fix
    case 'wait':
      return palette.waitBar
    default:
      return palette.signal
  }
}

const escaped = (text: string) => text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')

const WIDTH = 720
const LABEL = 180
const ROW = 22
const AXIS = 24

/** The chart as one SVG image in `palette`, its rows cut short where they would pass what a surface draws. */
export function svgOf(chart: Gantt, palette: Palette): string {
  const plot = WIDTH - LABEL - 8
  const x = (at: number) => (LABEL + ((at - chart.start) / (chart.end - chart.start)) * plot).toFixed(1)
  const w = (span: Span) => Math.max(2, ((span.to - span.from) / (chart.end - chart.start)) * plot).toFixed(1)
  const axis = ticksOf(chart, 6).map(
    tick =>
      `<line x1="${x(tick)}" x2="${x(tick)}" y1="${AXIS - 6}" y2="100%" stroke="${palette.line}"/><text x="${x(tick)}" y="14" fill="${palette.muted}" text-anchor="middle">${escaped(clockOf(new Date(tick).toISOString()))}</text>`,
  )
  const drawn: string[] = []
  let size = axis.join('').length + 600
  let shown = 0
  for (const row of chart.rows) {
    const y = AXIS + shown * ROW
    const isStage = row.kind === 'stage'
    const label = `<text x="${isStage ? 4 : 16}" y="${y + 15}" fill="${isStage ? palette.ink : palette.muted}"${isStage ? ' font-weight="600"' : ''}>${escaped(row.title.slice(0, 26))}</text>`
    const bar = isStage
      ? `<rect x="${x(row.from)}" y="${y + 5}" width="${w(row)}" height="12" fill="${toneColour(palette, row.tone)}"/>`
      : `<rect x="${x(row.from)}" y="${y + 7}" width="${w(row)}" height="8" fill="${toneColour(palette, row.tone)}"/>`
    const waits = row.waits.map(wait => `<rect x="${x(wait.from)}" y="${y + 5}" width="${w(wait)}" height="12" fill="${palette.waitBar}"/>`).join('')
    const piece = `<g>${label}${bar}${waits}</g>`
    if (size + piece.length > MAX_SVG - 400) break
    drawn.push(piece)
    size += piece.length
    shown++
  }
  const more = chart.rows.length - shown
  const tail = more > 0 ? `<text x="4" y="${AXIS + shown * ROW + 15}" fill="${palette.muted}">${more} more rows</text>` : ''
  const height = AXIS + (shown + (more > 0 ? 1 : 0)) * ROW + 4
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${WIDTH}" height="${height}" viewBox="0 0 ${WIDTH} ${height}" font-family="system-ui, -apple-system, sans-serif" font-size="12">${axis.join('')}${drawn.join('')}${tail}</svg>`
}

/** A stretch of a terminal bar in one tone, or blank. */
export type Run = { text: string; tone: Tone | 'wait' | null }

/** Each row as `width` columns of block characters, from the column of its start to that of its end. */
export function barsOf(chart: Gantt, width: number): { key: string; runs: Run[] }[] {
  const column = (at: number) => Math.round(((at - chart.start) / (chart.end - chart.start)) * (width - 1))
  return chart.rows.map(row => {
    const tones: (Tone | 'wait' | null)[] = Array.from({ length: width }, () => null)
    for (let n = column(row.from); n <= column(row.to); n++) tones[n] = row.tone
    for (const wait of row.waits) for (let n = column(wait.from); n <= column(wait.to); n++) tones[n] = 'wait'
    const runs: Run[] = []
    for (const tone of tones) {
      const glyph = tone === null ? ' ' : row.kind === 'stage' ? '█' : '▬'
      const last = runs.at(-1)
      if (last && last.tone === tone) last.text += glyph
      else runs.push({ text: glyph, tone })
    }
    return { key: row.key, runs }
  })
}
