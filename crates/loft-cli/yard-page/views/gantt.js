// The Gantt chart: a bar above it dividing the time that was not idle, then a lane for each
// agent along time, each stage with its tasks, Loft runs and views, waits on the person and the
// agent's own work; idle stretches longer than 45 seconds folded to a fixed width and labelled;
// and the stretches of plan and spec mode drawn across every lane. Choosing a stage, a task or a
// submission opens the timeline at it.

import { h, svg } from '../lib/dom.js';
import { length, when } from '../lib/format.js';
import { chart, scale } from '../model/gantt.js';

const LABEL = 168;
const STAGE = 16;
const TASK = 9;
const GAP = 4;
const ROW_PAD = 8;
const BAND_HEAD = 18;

/** About how wide one character of a label is drawn, in pixels. */
const CHARACTER = 6.5;

/**
 * An idle stretch's label, centred under it where it fits inside the chart, and otherwise held
 * inside the chart's edge it would cross.
 * @param {string} text
 * @param {number} from
 * @param {number} to
 * @param {number} width
 * @param {number} y
 */
function idleLabel(text, from, to, width, y) {
  const half = (text.length * CHARACTER) / 2;
  const centre = (from + to) / 2;
  const [x, anchor] =
    centre + half > width - 2 ? [Math.min(to, width) - 2, 'end'] : centre - half < 2 ? [Math.max(from, 0) + 2, 'start'] : [centre, 'middle'];
  return svg('text', { x, y, 'text-anchor': anchor, class: 'idle-label' }, text);
}

/**
 * The height a stage's row takes.
 * @param {any} row
 */
function rowHeight(row) {
  const extras = (row.loft.length || row.submissions.length || row.views.length) ? TASK + GAP : 0;
  return STAGE + GAP + row.tasks.length * (TASK + GAP) + extras + ROW_PAD;
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
  const inner = Math.max(240, width - LABEL - 8);
  const x = scale(model, inner);
  const X = (time) => LABEL + x(time);
  const laneHeights = model.lanes.map((lane) => lane.rows.reduce((sum, row) => sum + rowHeight(row), 0) + 10);
  const height = BAND_HEAD + laneHeights.reduce((a, b) => a + b, 0) + 38;
  const chartSvg = svg(
    'svg',
    { class: 'gantt-chart', width, height, viewBox: `0 0 ${width} ${height}`, role: 'img', 'aria-label': 'Yard Log along time, a lane for each agent' },
    svg(
      'defs',
      {},
      svg('pattern', { id: 'hatch-spec', width: 6, height: 6, patternUnits: 'userSpaceOnUse', patternTransform: 'rotate(45)' }, svg('rect', { width: 3, height: 6, class: 'hatch-spec' })),
      svg('pattern', { id: 'hatch-idle', width: 5, height: 5, patternUnits: 'userSpaceOnUse', patternTransform: 'rotate(-45)' }, svg('rect', { width: 1, height: 5, class: 'hatch-idle' })),
    ),
    model.idle
      .filter((span) => span.folded)
      .map((span) =>
        svg(
          'g',
          { class: 'idle-fold' },
          svg('rect', { x: X(span.from), y: BAND_HEAD, width: X(span.to) - X(span.from), height: height - BAND_HEAD - 34, fill: 'url(#hatch-idle)' }),
          idleLabel(`idle ${length(span.to - span.from)}`, X(span.from), X(span.to), width, height - 22),
        ),
      ),
    model.bands.map((band) =>
      svg(
        'g',
        { class: ['band', `band--${band.mode}`].join(' ') },
        svg('rect', {
          x: X(band.from),
          y: BAND_HEAD,
          width: Math.max(2, X(band.to) - X(band.from)),
          height: height - BAND_HEAD - 34,
          class: `band-fill band-fill--${band.mode}`,
          fill: band.mode === 'spec' ? 'url(#hatch-spec)' : null,
        }),
        svg(
          'text',
          { x: X(band.from) + 3, y: BAND_HEAD - 5, class: 'band-label' },
          `${band.mode === 'plan' ? 'Plan mode' : 'Spec mode'}, ${band.open ? `${length(band.to - band.from)} so far` : length(band.to - band.from)}`,
        ),
      ),
    ),
    lanes(model, laneHeights, X, onPick),
    axis(model, X, height),
  );
  return h('div', { class: 'gantt' }, summaryBar(model.summary), h('div', { class: 'gantt-scroll' }, chartSvg), legend());
}

/**
 * @param {any} model
 * @param {number[]} laneHeights
 * @param {(time: number) => number} X
 * @param {(id: string) => void} onPick
 */
function lanes(model, laneHeights, X, onPick) {
  let y = BAND_HEAD + 4;
  return model.lanes.map((lane, index) => {
    const top = y;
    y += laneHeights[index];
    let rowTop = top + 6;
    return svg(
      'g',
      { class: ['lane', lane.main ? 'lane--main' : 'lane--sub'].join(' '), dataset: null },
      svg('rect', { x: 0, y: top, width: '100%', height: laneHeights[index] - 4, class: 'lane-back' }),
      svg('text', { x: 8, y: top + 16, class: 'lane-label' }, lane.label),
      lane.rows.map((row) => {
        const at = rowTop;
        rowTop += rowHeight(row);
        return rowGroup(row, at, X, onPick);
      }),
    );
  });
}

/**
 * @param {any} row
 * @param {number} top
 * @param {(time: number) => number} X
 * @param {(id: string) => void} onPick
 */
function rowGroup(row, top, X, onPick) {
  const stage = row.stage;
  const width = (from, to) => Math.max(2, X(to) - X(from));
  const pick = (id) => ({ onclick: () => onPick(id), tabindex: 0, role: 'button', onkeydown: (event) => (event.key === 'Enter' ? onPick(id) : null) });
  const parts = [
    row.work.map((span) => svg('rect', { x: X(span.from), y: top, width: width(span.from, span.to), height: STAGE, class: 'work' })),
    svg(
      'g',
      { class: `stage-bar status--${stage.status}`, ...pick(stage.id) },
      svg('rect', { x: X(stage.from), y: top, width: width(stage.from, stage.to), height: STAGE, rx: 3, class: 'stage-rect' }),
      svg('text', { x: X(stage.from) + 4, y: top + 12, class: 'stage-label' }, stage.title),
      svg('title', {}, `${stage.title}: ${stage.status}, ${when(new Date(stage.from).toISOString())} for ${length(stage.to - stage.from)}`),
    ),
  ];
  let y = top + STAGE + GAP;
  for (const task of row.tasks) {
    const listedTo = task.activeFrom ?? task.to;
    parts.push(
      svg(
        'g',
        { class: ['task-bar', task.done ? 'is-done' : null].filter(Boolean).join(' '), ...pick(task.id) },
        svg('rect', { x: X(task.from), y, width: width(task.from, listedTo), height: TASK, class: 'task-listed' }),
        task.activeFrom !== null ? svg('rect', { x: X(task.activeFrom), y, width: width(task.activeFrom, task.to), height: TASK, class: 'task-active' }) : null,
        task.done ? svg('circle', { cx: X(task.to), cy: y + TASK / 2, r: 3, class: 'task-done' }) : null,
        svg('title', {}, `${task.title}${task.done ? ', done' : ''}`),
      ),
    );
    y += TASK + GAP;
  }
  if (row.loft.length || row.submissions.length || row.views.length) {
    for (const run of row.loft) {
      parts.push(svg('g', { class: 'loft-run' }, svg('rect', { x: X(run.from), y, width: width(run.from, run.to), height: TASK, class: run.failed ? 'loft-failed' : 'loft' }), svg('title', {}, `loft ${run.name}`)));
    }
    for (const wait of row.submissions) {
      parts.push(
        svg('g', { class: 'wait', ...(wait.block ? pick(wait.block) : {}) }, svg('rect', { x: X(wait.from), y, width: width(wait.from, wait.to), height: TASK, class: wait.open ? 'wait-open' : 'wait-done' }), svg('title', {}, `Waited on you for ${length(wait.to - wait.from)}`)),
      );
    }
    for (const view of row.views) {
      parts.push(svg('g', { class: 'loft-view-mark', ...pick(view.id) }, svg('path', { d: `M ${X(view.at)} ${y} l 4 ${TASK / 2} l -4 ${TASK / 2} l -4 -${TASK / 2} z`, class: 'view-mark' }), svg('title', {}, `loft view: ${view.view}`)));
    }
  }
  return svg('g', { class: 'stage-row' }, parts);
}

/**
 * Times along the bottom.
 * @param {any} model
 * @param {(time: number) => number} X
 * @param {number} height
 */
function axis(model, X, height) {
  // The start and the end, and the start of each stretch of work after a folded idle stretch,
  // each kept only where it leaves room for the one before it.
  const times = [model.start, ...model.idle.filter((span) => span.folded).map((span) => span.to), model.end];
  const ticks = [];
  for (const time of times) {
    const last = ticks[ticks.length - 1];
    if (!last || X(time) - X(last) >= 90) ticks.push(time);
    else if (time === model.end) ticks[ticks.length - 1] = time;
  }
  return svg(
    'g',
    { class: 'axis' },
    ticks.map((time, index) =>
      svg(
        'text',
        { x: X(time), y: height - 6, 'text-anchor': index === 0 ? 'start' : time === model.end ? 'end' : 'middle', class: 'tick' },
        when(new Date(time).toISOString()),
      ),
    ),
  );
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

function legend() {
  return h(
    'p',
    { class: 'gantt-legend' },
    'Stages are the wide bars, tasks the thin ones beneath, their listed time pale and their active time solid. ',
    'Loft runs and waits on you sit under their stage, and a diamond marks each Loft view recorded. ',
    'Plan mode is a pink band and spec mode a hatched one.',
  );
}
