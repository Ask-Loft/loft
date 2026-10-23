// The Gantt chart: a bar above it dividing the time that was not idle, then the chart itself.
// Along its top run the time axis and the track of plan and spec mode; beneath them a lane for
// the main agent and one for each subagent, each stage's bar divided into its active tasks, Loft
// runs, waits on the person and agent work, with its tasks beneath it. A column beside the chart
// names each lane, stage and task, idle stretches longer than 45 seconds are folded into breaks
// of the axis, and no label crosses another or the chart's edge. Choosing a stage, a task or a
// submission opens the timeline at it.

import { h, svg } from '../lib/dom.js';
import { length, when } from '../lib/format.js';
import { chart, scale, ticks } from '../model/gantt.js';

/** About how wide one character of a label is drawn, in pixels. */
export const CHARACTER = 7;

const AXIS = 22;
const MODE = 12;
const TOP = AXIS + MODE + 6;
const LANE_HEAD = 24;
const LANE_PAD = 8;
const STAGE_ROW = 22;
const STAGE = 14;
const TASK_ROW = 15;
const TASK = 8;
const INDENT = 14;
const EDGE = 8;
const FOLD = 28;

/**
 * `text` cut short with an ellipsis to fit `room` pixels.
 * @param {string} text
 * @param {number} room
 */
function fit(text, room) {
  const most = Math.max(1, Math.floor(room / CHARACTER));
  return text.length <= most ? text : `${text.slice(0, most - 1).trimEnd()}…`;
}

/** @param {number} room */
function columnWidth(room) {
  return Math.round(Math.min(300, Math.max(180, room * 0.22)));
}

/**
 * The height a lane takes.
 * @param {any} lane
 */
function laneHeight(lane) {
  return LANE_HEAD + lane.rows.reduce((sum, row) => sum + STAGE_ROW + row.tasks.length * TASK_ROW, 0) + LANE_PAD;
}

/**
 * The chart of `log`, `width` wide, at `now`; `onPick` opens the timeline at a block.
 * @param {any} log
 * @param {{ width: number, now: number, onPick: (id: string) => void }} options
 */
export function gantt(log, { width, now, onPick }) {
  const model = chart(log, now);
  if (!model.lanes.length) {
    return h('p', { class: 'empty' }, 'No stage has started yet, so there is nothing to draw along time.');
  }
  const names = columnWidth(width);
  const left = names + EDGE;
  const inner = Math.max(120, width - left - EDGE);
  const x = scale(model, inner, FOLD);
  const X = (time) => left + x(time);
  const heights = model.lanes.map(laneHeight);
  const bottom = TOP + heights.reduce((a, b) => a + b, 0);
  const height = bottom + 6;
  const pick = (id) => ({ onclick: () => onPick(id), tabindex: 0, role: 'button', onkeydown: (event) => (event.key === 'Enter' ? onPick(id) : null) });
  const chartSvg = svg(
    'svg',
    { class: 'gantt-chart', width, height, viewBox: `0 0 ${width} ${height}`, role: 'img', 'aria-label': 'Yard Log along time, a lane for each agent' },
    svg(
      'defs',
      {},
      svg('pattern', { id: 'hatch-spec', width: 6, height: 6, patternUnits: 'userSpaceOnUse', patternTransform: 'rotate(45)' }, svg('rect', { width: 3, height: 6, class: 'hatch-spec' })),
    ),
    laneBacks(model, heights, width),
    washes(model, X, bottom),
    axis(model, x, X, left, width),
    modes(model, X),
    folds(model, X, bottom),
    lanes(model, heights, { X, names, width, pick }),
  );
  return h('div', { class: 'gantt' }, summaryBar(model.summary), h('div', { class: 'gantt-scroll' }, chartSvg));
}

/**
 * Each lane's background, beneath everything else.
 * @param {any} model
 * @param {number[]} heights
 * @param {number} width
 */
function laneBacks(model, heights, width) {
  let y = TOP;
  return model.lanes.map((lane, index) => {
    const top = y;
    y += heights[index];
    return svg('rect', { x: 0, y: top, width, height: heights[index] - 2, class: ['lane-back', index % 2 ? 'lane-back--odd' : null].filter(Boolean).join(' ') });
  });
}

/**
 * Each stretch of plan and spec mode shaded faintly down every lane, over their backgrounds.
 * @param {any} model
 * @param {(time: number) => number} X
 * @param {number} bottom
 */
function washes(model, X, bottom) {
  return model.bands.map((band) =>
    svg('rect', {
      x: X(band.from),
      y: TOP,
      width: Math.max(2, X(band.to) - X(band.from)),
      height: bottom - TOP,
      class: `band-fill band-fill--${band.mode}`,
      fill: band.mode === 'spec' ? 'url(#hatch-spec)' : null,
    }),
  );
}

/**
 * The time axis along the top: its line and a label at each of its marks.
 * @param {any} model
 * @param {(time: number) => number} x
 * @param {(time: number) => number} X
 * @param {number} left
 * @param {number} width
 */
function axis(model, x, X, left, width) {
  const marks = ticks(model, x, 84);
  return svg(
    'g',
    { class: 'axis' },
    svg('line', { x1: left, x2: width - EDGE, y1: AXIS - 2, y2: AXIS - 2, class: 'axis-line' }),
    marks.map((time, index) => {
      const at = X(time);
      const anchor = index === 0 ? 'start' : index === marks.length - 1 && at > width - EDGE - 40 ? 'end' : 'middle';
      return svg(
        'g',
        { class: 'tick-group' },
        svg('line', { x1: at, x2: at, y1: AXIS - 6, y2: AXIS - 2, class: 'tick-mark' }),
        svg('text', { x: at, y: AXIS - 9, 'text-anchor': anchor, class: 'tick' }, when(new Date(time).toISOString())),
      );
    }),
  );
}

/**
 * The track of modes: each stretch of plan and spec mode, labelled where the label fits.
 * @param {any} model
 * @param {(time: number) => number} X
 */
function modes(model, X) {
  return model.bands.map((band) => {
    const from = X(band.from);
    const wide = Math.max(2, X(band.to) - from);
    const name = band.mode === 'plan' ? 'Plan mode' : 'Spec mode';
    const told = `${name}, ${length(band.to - band.from)}${band.open ? ' so far' : ''}`;
    return svg(
      'g',
      { class: `mode mode--${band.mode}` },
      svg('rect', { x: from, y: AXIS, width: wide, height: MODE, class: 'mode-fill', fill: band.mode === 'spec' ? 'url(#hatch-spec)' : null }),
      name.length * CHARACTER <= wide - 6 ? svg('text', { x: from + 3, y: AXIS + MODE - 2, class: 'band-label' }, name) : null,
      svg('title', {}, told),
    );
  });
}

/**
 * Each folded idle stretch: a gap down the chart and a break in the axis and the track.
 * @param {any} model
 * @param {(time: number) => number} X
 * @param {number} bottom
 */
function folds(model, X, bottom) {
  return model.idle
    .filter((span) => span.folded)
    .map((span) => {
      const from = X(span.from);
      const to = X(span.to);
      const zig = (at) => `M ${at} ${AXIS - 8} l 3 3 l -6 4 l 6 4 l -6 4 l 6 4 l -3 3`;
      return svg(
        'g',
        { class: 'idle-fold' },
        svg('rect', { x: from, y: AXIS - 8, width: to - from, height: bottom - AXIS + 8, class: 'idle-gap' }),
        svg('path', { d: zig((from + to) / 2), class: 'break-mark' }),
        svg('title', {}, `idle ${length(span.to - span.from)}`),
      );
    });
}

/**
 * The lanes: each one's name, its sessions' rules, and its rows.
 * @param {any} model
 * @param {number[]} heights
 * @param {{ X: (time: number) => number, names: number, width: number, pick: (id: string) => object }} draw
 */
function lanes(model, heights, draw) {
  let y = TOP;
  return model.lanes.map((lane, index) => {
    const top = y;
    y += heights[index];
    let rowTop = top + LANE_HEAD;
    return svg(
      'g',
      { class: ['lane', lane.main ? 'lane--main' : 'lane--sub'].join(' ') },
      svg('text', { x: EDGE, y: top + 16, class: 'lane-label' }, fit(lane.label, draw.names - EDGE * 2)),
      sessionRules(lane, top, heights[index], draw),
      lane.rows.map((row) => {
        const at = rowTop;
        rowTop += STAGE_ROW + row.tasks.length * TASK_ROW;
        return rowGroup(row, at, draw);
      }),
    );
  });
}

/**
 * A rule at the start of each session of the main agent after the first, named where its name
 * fits before the next rule and the chart's edge.
 * @param {any} lane
 * @param {number} top
 * @param {number} height
 * @param {{ X: (time: number) => number, width: number }} draw
 */
function sessionRules(lane, top, height, { X, width }) {
  const later = (lane.sessions ?? []).filter((session) => session.number > 1);
  return later.map((session, index) => {
    const at = X(session.from);
    const next = later[index + 1];
    const room = (next ? X(next.from) : width - EDGE) - at - 8;
    const name = `Session ${session.number} · ${when(new Date(session.from).toISOString())}`;
    return svg(
      'g',
      { class: 'session' },
      svg('line', { x1: at, x2: at, y1: top + 2, y2: top + height - 4, class: 'session-rule' }),
      name.length * CHARACTER <= room ? svg('text', { x: at + 4, y: top + 16, class: 'session-label' }, name) : null,
      svg('title', {}, `Session ${session.number}: ${session.session}`),
    );
  });
}

/**
 * One stage's rows: its name and bar, then each task's.
 * @param {any} row
 * @param {number} top
 * @param {{ X: (time: number) => number, names: number, pick: (id: string) => object }} draw
 */
function rowGroup(row, top, { X, names, pick }) {
  const stage = row.stage;
  const width = (from, to) => Math.max(2, X(to) - X(from));
  const barTop = top + (STAGE_ROW - STAGE) / 2;
  const middle = barTop + STAGE / 2;
  const runs = (segment) => row.loft.filter((run) => run.from < segment.to && run.to > segment.from);
  const parts = [
    svg('g', { class: 'stage-name', ...pick(stage.id) }, svg('text', { x: EDGE + 4, y: middle + 4 }, fit(stage.title, names - EDGE * 2 - 4)), svg('title', {}, stage.title)),
    svg(
      'g',
      { class: `stage-bar status--${stage.status}`, ...pick(stage.id) },
      svg('rect', { x: X(stage.from), y: barTop, width: width(stage.from, stage.to), height: STAGE, rx: 2, class: 'stage-back' }),
      row.segments
        .filter((segment) => segment.kind !== 'wait')
        .map((segment) =>
          svg(
            'rect',
            { x: X(segment.from), y: barTop, width: width(segment.from, segment.to), height: STAGE, class: `seg seg--${segment.kind}${segment.kind === 'loft' && runs(segment).some((run) => run.failed) ? ' seg--failed' : ''}` },
            segment.kind === 'loft' ? svg('title', {}, runs(segment).map((run) => `loft ${run.name}`).join(', ')) : null,
          ),
        ),
      svg('title', {}, `${stage.title}: ${stage.status}, ${when(new Date(stage.from).toISOString())} for ${length(stage.to - stage.from)}`),
    ),
    row.segments
      .filter((segment) => segment.kind === 'wait')
      .map((segment) =>
        svg(
          'g',
          { class: 'wait', ...(segment.block ? pick(segment.block) : {}) },
          svg('rect', { x: X(segment.from), y: barTop, width: width(segment.from, segment.to), height: STAGE, class: 'seg seg--wait' }),
          svg('title', {}, `Waited on you for ${length(segment.to - segment.from)}`),
        ),
      ),
    svg('rect', { x: X(stage.from), y: barTop, width: width(stage.from, stage.to), height: STAGE, rx: 2, class: `stage-rect status--${stage.status}` }),
    row.views.map((view) =>
      svg(
        'g',
        { class: 'loft-view-mark', ...pick(view.id) },
        svg('path', { d: `M ${X(view.at)} ${middle - 5} l 3.5 5 l -3.5 5 l -3.5 -5 z`, class: 'view-mark' }),
        svg('title', {}, `loft view: ${view.view}`),
      ),
    ),
  ];
  let y = top + STAGE_ROW;
  for (const task of row.tasks) {
    const listedTo = task.activeFrom ?? task.to;
    const taskTop = y + (TASK_ROW - TASK) / 2;
    parts.push(
      svg('g', { class: 'task-name', ...pick(task.id) }, svg('text', { x: EDGE + 4 + INDENT, y: taskTop + TASK / 2 + 4 }, fit(task.title, names - EDGE * 2 - 4 - INDENT)), svg('title', {}, task.title)),
      svg(
        'g',
        { class: ['task-bar', task.done ? 'is-done' : null].filter(Boolean).join(' '), ...pick(task.id) },
        svg('rect', { x: X(task.from), y: taskTop, width: width(task.from, listedTo), height: TASK, class: 'task-listed' }),
        task.activeFrom !== null ? svg('rect', { x: X(task.activeFrom), y: taskTop, width: width(task.activeFrom, task.to), height: TASK, class: 'task-active' }) : null,
        task.done ? svg('circle', { cx: X(task.to), cy: taskTop + TASK / 2, r: 3, class: 'task-done' }) : null,
        svg('title', {}, `${task.title}${task.done ? ', done' : ''}`),
      ),
    );
    y += TASK_ROW;
  }
  return svg('g', { class: 'stage-row' }, parts);
}

/**
 * The bar above the chart: where the time that was not idle went.
 * @param {{ total: number, parts: { name: string, kind: string, length: number, share: number }[] }} summary
 */
function summaryBar(summary) {
  return h(
    'div',
    { class: 'time-split' },
    h(
      'div',
      { class: 'split-bar', role: 'img', 'aria-label': summary.parts.map((part) => `${part.name} ${Math.round(part.share * 100)}%`).join(', ') },
      summary.parts.map((part) => h('span', { class: `split split--${part.kind}`, style: `width:${part.share * 100}%` })),
    ),
    h(
      'ul',
      { class: 'split-legend' },
      summary.parts.map((part) =>
        h('li', {}, h('span', { class: `swatch split--${part.kind}` }), `${part.name}: ${length(part.length)}, ${Math.round(part.share * 100)}%`),
      ),
      h('li', { class: 'split-total' }, `Not idle: ${length(summary.total)}`),
    ),
  );
}
