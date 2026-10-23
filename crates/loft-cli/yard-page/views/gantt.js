// The Gantt chart, in HTML: a bar above it dividing the time that was not idle, then a frame
// that scrolls within the page. In the frame, the time axis and the track of plan and spec mode
// stay at the top and a column naming each lane, stage and task stays at the left; beneath them a
// lane for the main agent and one for each subagent, each stage's bar divided into its active
// tasks, Loft runs, waits on the person and agent work, with its tasks beneath it, over a faint
// grid. Every time is placed as a share of the chart's width and an offset in pixels, so the bars
// follow the width the browser gives them. The column's edge can be dragged, and its width is
// kept in this browser. Choosing a stage, a task or a submission opens the timeline at it.

import { h } from '../lib/dom.js';
import { length, when } from '../lib/format.js';
import * as prefs from '../lib/prefs.js';
import { chart, minimumWidth, place, ticks } from '../model/gantt.js';

/** The width a folded idle stretch is drawn, in pixels. */
const FOLD = 28;

/** The column of names: its narrowest, its widest and its width until one is chosen. */
export const NAMES = { least: 160, most: 480, first: 260 };

/** About how wide one character of a small label is drawn, in pixels. */
const LETTER = 6.5;

/** How far an arrow key moves the column's edge, in pixels. */
const STEP = 16;

/** Where each chart's frame was scrolled to, by its Yard Log, for as long as the page is open. */
const scrolls = new Map();

/**
 * The column's width once its edge has moved `moved` pixels from `start`.
 * @param {number} start
 * @param {number} moved
 */
export function columnWidth(start, moved) {
  return Math.round(Math.min(NAMES.most, Math.max(NAMES.least, start + moved)));
}

/** The column's width kept in this browser, or its first width. */
function keptWidth() {
  const kept = Number(prefs.get('gantt-names'));
  return Number.isFinite(kept) && kept > 0 ? columnWidth(kept, 0) : NAMES.first;
}

/**
 * A length as CSS gives it: a share of the track's width and pixels.
 * @param {{ share: number, offset: number }} at
 */
function css({ share, offset }) {
  const percent = Number((share * 100).toFixed(4));
  const pixels = Number(offset.toFixed(2));
  return `calc(${percent}% ${pixels < 0 ? '-' : '+'} ${Math.abs(pixels)}px)`;
}

/**
 * @typedef {{
 *   at: (time: number) => { share: number, offset: number },
 *   px: (time: number) => number,
 *   span: (from: number, to: number) => string,
 *   left: (time: number) => string,
 *   track: number,
 *   pick: (id: string) => object,
 * }} Draw
 */

/**
 * The chart of `log`, on a page `width` wide, at `now`; `onPick` opens the timeline at a block.
 * @param {any} log
 * @param {{ width: number, now: number, onPick: (id: string) => void }} options
 * @returns {HTMLElement}
 */
export function gantt(log, options) {
  const { width, now, onPick } = options;
  const model = chart(log, now);
  if (!model.lanes.length) {
    return h('p', { class: 'empty' }, 'No stage has started yet, so there is nothing to draw along time.');
  }
  const names = keptWidth();
  const least = minimumWidth(model, FOLD);
  const track = Math.max(width - names - 2, least);
  const at = place(model, FOLD);
  /** @type {Draw} */
  const draw = {
    at,
    px: (time) => at(time).share * track + at(time).offset,
    left: (time) => `left: ${css(at(time))}`,
    span: (from, to) => {
      const start = at(from);
      const end = at(to);
      return `left: ${css(start)}; width: ${css({ share: end.share - start.share, offset: end.offset - start.offset })}`;
    },
    track,
    pick: (id) => ({ onclick: () => onPick(id) }),
  };
  const marks = labels(model, ticks(model, (time) => draw.px(time), 84), draw);
  const key = `${log?.tree ?? ''}\n${log?.branch ?? ''}`;
  const frame = h(
    'div',
    {
      class: 'gantt-frame',
      style: `--names: ${names}px`,
      dataset: { key },
      onscroll: (/** @type {Event} */ event) => {
        const target = /** @type {HTMLElement} */ (event.currentTarget);
        scrolls.set(key, { left: target.scrollLeft, top: target.scrollTop });
      },
    },
    h(
      'div',
      { class: 'gantt-sheet', style: `width: max(100%, calc(var(--names) + ${least}px))` },
      head(model, marks, draw),
      model.lanes.map((lane, index) => laneBlock(model, lane, index, marks.map((mark) => mark.time), draw)),
    ),
  );
  const view = h('div', { class: 'gantt' }, summaryBar(model.summary), frame);
  const corner = frame.querySelector('.gantt-corner');
  corner?.append(edge(frame, names, () => {
    const again = gantt(log, options);
    view.replaceWith(again);
    settle(again);
    /** @type {HTMLElement | null} */ (again.querySelector('.names-edge'))?.focus();
  }));
  return view;
}

/**
 * Scrolls each chart in `root` to where it was, or, shown for the first time, to its latest time.
 * @param {ParentNode} root
 */
export function settle(root) {
  for (const frame of /** @type {NodeListOf<HTMLElement>} */ (root.querySelectorAll('.gantt-frame'))) {
    const key = frame.dataset.key ?? '';
    const kept = scrolls.get(key);
    if (kept) {
      frame.scrollLeft = kept.left;
      frame.scrollTop = kept.top;
    } else {
      frame.scrollLeft = frame.scrollWidth;
      scrolls.set(key, { left: frame.scrollLeft, top: frame.scrollTop });
    }
  }
}

/**
 * The column's edge: dragged, or moved with the arrow keys, it sets the column's width, kept in
 * this browser once the drag ends.
 * @param {HTMLElement} frame
 * @param {number} start
 * @param {() => void} done
 */
function edge(frame, start, done) {
  let width = start;
  let from = /** @type {number | null} */ (null);
  let grabbed = start;
  const handle = h('div', {
    class: 'names-edge',
    role: 'separator',
    tabindex: 0,
    'aria-orientation': 'vertical',
    'aria-label': 'Width of the column of names',
    'aria-valuemin': NAMES.least,
    'aria-valuemax': NAMES.most,
    'aria-valuenow': start,
  });
  const show = (next) => {
    width = next;
    frame.style.setProperty('--names', `${next}px`);
    handle.setAttribute('aria-valuenow', String(next));
  };
  const keep = () => {
    prefs.set('gantt-names', String(width));
    done();
  };
  handle.addEventListener('pointerdown', (/** @type {any} */ event) => {
    from = event.clientX;
    grabbed = width;
    handle.setPointerCapture?.(event.pointerId);
    event.preventDefault();
  });
  handle.addEventListener('pointermove', (/** @type {any} */ event) => {
    if (from !== null) show(columnWidth(grabbed, event.clientX - from));
  });
  handle.addEventListener('pointerup', () => {
    if (from === null) return;
    from = null;
    keep();
  });
  handle.addEventListener('keydown', (/** @type {any} */ event) => {
    const moved = event.key === 'ArrowLeft' ? -STEP : event.key === 'ArrowRight' ? STEP : 0;
    if (!moved) return;
    event.preventDefault();
    show(columnWidth(width, moved));
    keep();
  });
  return handle;
}

/**
 * The label of each mark of the time axis, anchored so that it stays inside the chart, a mark
 * where time resumes after a folded idle stretch beginning just after its break, and kept only
 * where it crosses no break.
 * @param {any} model
 * @param {number[]} marks
 * @param {Draw} draw
 */
function labels(model, marks, draw) {
  const folded = model.idle.filter((span) => span.folded);
  const breaks = folded.map((span) => (draw.px(span.from) + draw.px(span.to)) / 2);
  const resumes = new Set(folded.map((span) => span.to));
  return marks
    .map((time, index) => {
      const x = draw.px(time);
      const text = when(new Date(time).toISOString());
      const wide = text.length * LETTER + 6;
      const anchor =
        index === 0 || resumes.has(time) ? 'start' : index === marks.length - 1 && x > draw.track - wide / 2 ? 'end' : 'middle';
      const [from, to] = anchor === 'start' ? [x, x + wide] : anchor === 'end' ? [x - wide, x] : [x - wide / 2, x + wide / 2];
      return { time, anchor, text, clear: breaks.every((at) => at < from - 4 || at > to + 4) };
    })
    .filter((label) => label.clear);
}

/**
 * The head of the frame, held at its top: the corner above the column, the time axis with a break
 * at each folded idle stretch, and the track of modes.
 * @param {any} model
 * @param {ReturnType<typeof labels>} marks
 * @param {Draw} draw
 */
function head(model, marks, draw) {
  return h(
    'div',
    { class: 'gantt-head' },
    h('div', { class: 'gantt-corner' }),
    h(
      'div',
      { class: 'gantt-axis' },
      marks.map(({ time, anchor, text }) => h('span', { class: `tick tick--${anchor}`, style: draw.left(time) }, text)),
      model.idle
        .filter((span) => span.folded)
        .map((span) =>
          h('span', { class: 'idle-fold', style: draw.span(span.from, span.to), title: `idle ${length(span.to - span.from)}` }, h('span', { class: 'break-mark' })),
        ),
      h(
        'div',
        { class: 'mode-track' },
        model.bands.map((band) => {
          const name = band.mode === 'plan' ? 'Plan mode' : 'Spec mode';
          const wide = draw.px(band.to) - draw.px(band.from);
          return h(
            'span',
            { class: `mode mode--${band.mode}`, style: draw.span(band.from, band.to), title: `${name}, ${length(band.to - band.from)}${band.open ? ' so far' : ''}` },
            name.length * LETTER <= wide - 6 ? h('span', { class: 'band-label' }, name) : null,
          );
        }),
      ),
    ),
  );
}

/**
 * One lane: its overlay of washes, grid lines, idle gaps and session rules beneath the bars, its
 * name and session labels, and a row for each stage and each task.
 * @param {any} model
 * @param {any} lane
 * @param {number} index
 * @param {number[]} marks
 * @param {Draw} draw
 */
function laneBlock(model, lane, index, marks, draw) {
  const sessions = lane.sessions ?? [];
  return h(
    'div',
    { class: ['gantt-lane', lane.main ? 'lane--main' : 'lane--sub', index % 2 ? 'lane--odd' : null] },
    h(
      'div',
      { class: 'lane-overlay' },
      model.bands.map((band) => h('span', { class: `band-fill band-fill--${band.mode}`, style: draw.span(band.from, band.to) })),
      marks.map((time) => h('span', { class: 'grid-line', style: draw.left(time) })),
      model.idle.filter((span) => span.folded).map((span) => h('span', { class: 'idle-gap', style: draw.span(span.from, span.to) })),
      sessions.map((session) =>
        h('span', { class: 'session-rule', style: draw.left(session.from), title: `Session ${session.number}: ${session.session}` }),
      ),
    ),
    h(
      'div',
      { class: 'gantt-row lane-head' },
      h('div', { class: 'name-cell' }, h('span', { class: 'lane-label', title: lane.label }, lane.label)),
      h(
        'div',
        { class: 'track-cell' },
        sessions.map((session, at) => {
          const next = sessions[at + 1];
          const room = (next ? draw.px(next.from) : draw.track) - draw.px(session.from) - 8;
          const name = `Session ${session.number} · ${when(new Date(session.from).toISOString())}`;
          return name.length * LETTER <= room
            ? h('span', { class: 'session-label', style: draw.left(session.from), title: `Session ${session.number}: ${session.session}` }, name)
            : null;
        }),
      ),
    ),
    lane.rows.map((row) => stageRows(row, draw)),
  );
}

/**
 * A stage's row, then a row for each of its tasks.
 * @param {any} row
 * @param {Draw} draw
 */
function stageRows(row, draw) {
  const stage = row.stage;
  const runs = (segment) => row.loft.filter((run) => run.from < segment.to && run.to > segment.from);
  return [
    h(
      'div',
      { class: 'gantt-row stage-row' },
      h('div', { class: 'name-cell' }, h('button', { type: 'button', class: 'stage-name', title: stage.title, ...draw.pick(stage.id) }, stage.title)),
      h(
        'div',
        { class: 'track-cell' },
        h('span', { class: 'stage-back', style: draw.span(stage.from, stage.to) }),
        row.segments
          .filter((segment) => segment.kind !== 'wait' && segment.kind !== 'loft')
          .map((segment) => h('span', { class: `seg seg--${segment.kind}`, style: draw.span(segment.from, segment.to) })),
        h('button', {
          type: 'button',
          class: `stage-bar status--${stage.status}`,
          style: draw.span(stage.from, stage.to),
          title: `${stage.title}: ${stage.status}, ${when(new Date(stage.from).toISOString())} for ${length(stage.to - stage.from)}`,
          ...draw.pick(stage.id),
        }),
        row.segments
          .filter((segment) => segment.kind === 'loft')
          .map((segment) =>
            h('span', {
              class: ['seg', 'seg--loft', runs(segment).some((run) => run.failed) ? 'seg--failed' : null],
              style: draw.span(segment.from, segment.to),
              title: runs(segment).map((run) => `loft ${run.name}`).join(', '),
              ...draw.pick(stage.id),
            }),
          ),
        row.segments
          .filter((segment) => segment.kind === 'wait')
          .map((segment) =>
            h('button', {
              type: 'button',
              class: 'wait seg seg--wait',
              style: draw.span(segment.from, segment.to),
              title: `Waited on you for ${length(segment.to - segment.from)}`,
              ...(segment.block ? draw.pick(segment.block) : {}),
            }),
          ),
        row.views.map((view) =>
          h('button', { type: 'button', class: 'loft-view-mark', style: draw.left(view.at), title: `loft view: ${view.view}`, ...draw.pick(view.id) }),
        ),
      ),
    ),
    row.tasks.map((task) => {
      const listedTo = task.activeFrom ?? task.to;
      return h(
        'div',
        { class: 'gantt-row task-row' },
        h('div', { class: 'name-cell' }, h('button', { type: 'button', class: 'task-name', title: task.title, ...draw.pick(task.id) }, task.title)),
        h(
          'div',
          { class: 'track-cell' },
          h('span', { class: 'task-listed', style: draw.span(task.from, listedTo) }),
          task.activeFrom !== null ? h('span', { class: 'task-active', style: draw.span(task.activeFrom, task.to) }) : null,
          task.done ? h('span', { class: 'task-done', style: draw.left(task.to) }) : null,
          h('button', {
            type: 'button',
            class: ['task-bar', task.done ? 'is-done' : null],
            style: draw.span(task.from, task.to),
            title: `${task.title}${task.done ? ', done' : ''}`,
            ...draw.pick(task.id),
          }),
        ),
      );
    }),
  ];
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
