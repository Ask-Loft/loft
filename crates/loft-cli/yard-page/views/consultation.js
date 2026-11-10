// What the agents consulted, as the dataviz method draws it with no library: a legend of the
// classes in one row, then a chart of stacked bars of each stage's reads in Yard Log's order, each
// marked with whether the specification was consulted before the code was first edited; the same
// bars for the main agent and each type of subagent; and one bar of the share of the estimated
// context tokens each class brought in. Each class keeps one hue in every chart and both themes,
// the specification's the most prominent. Each segment names its class, count and share when
// pointed at or focused, and each chart has a table of its counts beneath it. The page sums
// nothing: it draws the sums the page server gives.

import { h, svg } from '../lib/dom.js';
import { count } from '../lib/format.js';
import { CLASSES, READS, summary } from '../model/consultation.js';

/** @typedef {import('../model/consultation.js').Counts} Counts */

/**
 * The consultation view of `log`.
 * @param {any} log
 */
export function consultationView(log) {
  const sums = summary(log.consultationSums);
  const unmeasured = unmeasuredNote(sums.unmeasured);
  if (!sums.measured) {
    return h(
      'section',
      { class: 'consultation', 'aria-label': 'What the agents consulted' },
      h('p', { class: 'empty' }, 'Nothing has been measured yet: no session’s adapter counted what its agents consulted.'),
      unmeasured,
    );
  }
  // Each mark is named for a reader by its own label; the tip is for the eye.
  const tip = h('div', { class: 'consult-tip', 'aria-hidden': 'true', hidden: true });
  const title = (/** @type {any} */ stage) => stage.title ?? stage.stage;
  return h(
    'section',
    { class: 'consultation', 'aria-label': 'What the agents consulted' },
    legend(READS),
    chart('By stage', [...sums.stages.map((stage) => ({ name: title(stage), counts: stage.counts, order: stage })), ...(sums.noStage ? [{ name: 'No stage', counts: sums.noStage.counts, order: sums.noStage }] : [])], tip),
    chart('By agent', sums.agents.map((agent) => ({ name: agent.type === 'main' ? 'main agent' : `${agent.type} subagent`, counts: agent.counts })), tip),
    estimate(sums.all?.estimate, sums.partial, tip),
    unmeasured,
    tip,
  );
}

/**
 * A legend of `classes` in one row, each a swatch of its hue and its words in ink.
 * @param {readonly (readonly [string, string])[]} classes
 */
function legend(classes) {
  return h(
    'ul',
    { class: 'consult-legend', 'aria-label': 'Classes' },
    classes.map(([key, words]) => h('li', {}, h('span', { class: `consult-swatch consult--${key}`, 'aria-hidden': 'true' }), words)),
  );
}

/**
 * Shows `text` in the tip beside `mark`, or hides it where `text` is none.
 * @param {HTMLElement} tip
 * @param {Element} mark
 * @param {string | null} text
 */
function point(tip, mark, text) {
  tip.hidden = !text;
  tip.textContent = text ?? '';
  if (!text) return;
  const view = tip.parentElement?.getBoundingClientRect();
  const at = mark.getBoundingClientRect();
  tip.style.left = `${Math.max(0, at.left - (view?.left ?? 0))}px`;
  tip.style.top = `${at.bottom - (view?.top ?? 0) + 4}px`;
}

/**
 * One bar of `parts`, each a segment as long as its share of `scale`, with a gap of the surface
 * between them; a segment names its class, count and share when pointed at or focused.
 * @param {{ key: string, words: string, value: number, said: string }[]} parts
 * @param {number} scale
 * @param {HTMLElement} tip
 */
function bar(parts, scale, tip) {
  let x = 0;
  const segments = parts
    .filter((part) => part.value > 0)
    .map((part) => {
      const width = (part.value / scale) * 100;
      const segment = svg('rect', {
        class: `consult-mark consult--${part.key}`,
        x: `${x}%`,
        y: 0,
        width: `${width}%`,
        height: '100%',
        rx: 2,
        tabindex: 0,
        role: 'img',
        'aria-label': part.said,
        onpointerenter: (/** @type {Event} */ event) => point(tip, /** @type {Element} */ (event.currentTarget), part.said),
        onpointerleave: () => point(tip, tip, null),
        onfocus: (/** @type {Event} */ event) => point(tip, /** @type {Element} */ (event.currentTarget), part.said),
        onblur: () => point(tip, tip, null),
      });
      x += width;
      return segment;
    });
  return svg('svg', { class: 'consult-bar', width: '100%', height: 16, 'aria-hidden': segments.length ? null : 'true' }, segments);
}

/**
 * The parts of one row of reads, each said in words with its share of the row.
 * @param {Counts} counts
 */
function readParts(counts) {
  const sum = READS.reduce((all, [key]) => all + counts[key], 0);
  return READS.map(([key, words]) => ({
    key,
    words,
    value: counts[key],
    said: `${words}: ${counts[key]} of ${sum} (${percent(counts[key], sum)})`,
  }));
}

/** @param {number} part @param {number} whole */
function percent(part, whole) {
  return whole ? `${Math.round((part / whole) * 100)}%` : '0%';
}

/**
 * The mark of whether a stage's specification was consulted before its code was first edited.
 * @param {{ specFirst?: boolean, codeEditsBeforeSpec?: number } | undefined} order
 */
function orderMark(order) {
  if (order?.specFirst === true) return h('span', { class: 'consult-order consult-order--spec-first' }, '✓ spec first');
  if (order?.specFirst === false) {
    const before = order.codeEditsBeforeSpec ?? 0;
    return h('span', { class: 'consult-order consult-order--code-first' }, `✗ code first, ${count(before, 'edit')}`);
  }
  return null;
}

/**
 * A chart of stacked bars, a row to each of `rows`, all on one scale, with its table beneath.
 * @param {string} title
 * @param {{ name: string, counts: Counts, order?: any }[]} rows
 * @param {HTMLElement} tip
 */
function chart(title, rows, tip) {
  const totals = rows.map((row) => READS.reduce((all, [key]) => all + row.counts[key], 0));
  const scale = Math.max(1, ...totals);
  return h(
    'figure',
    { class: 'consult-chart' },
    h('figcaption', {}, h('h4', {}, title)),
    rows.length ? null : h('p', { class: 'empty' }, 'No call has been counted here yet.'),
    h(
      'div',
      { class: 'consult-rows' },
      rows.map((row, index) =>
        h(
          'div',
          { class: 'consult-row' },
          h('span', { class: 'consult-name', title: row.name }, row.name),
          bar(readParts(row.counts), scale, tip),
          h('span', { class: 'consult-total' }, String(totals[index]), row.order ? orderMark(row.order) : null),
        ),
      ),
    ),
    table(title, rows),
  );
}

/**
 * The counts of a chart as a table, its text alternative.
 * @param {string} title
 * @param {{ name: string, counts: Counts, order?: any }[]} rows
 */
function table(title, rows) {
  const ordered = rows.some((row) => row.order);
  return h(
    'details',
    { class: 'consult-table' },
    h('summary', {}, `${title}, as a table`),
    h(
      'div',
      { class: 'table-wrap' },
      h(
        'table',
        { class: 'tokens-table' },
        h(
          'thead',
          {},
          h(
            'tr',
            {},
            h('td', {}),
            READS.map(([, words]) => h('th', { scope: 'col', class: 'number' }, words)),
            h('th', { scope: 'col', class: 'number' }, 'spec edits'),
            h('th', { scope: 'col', class: 'number' }, 'code edits'),
            ordered ? h('th', { scope: 'col' }, 'Order') : null,
          ),
        ),
        h(
          'tbody',
          {},
          rows.map((row) =>
            h(
              'tr',
              {},
              h('th', { scope: 'row' }, row.name),
              READS.map(([key]) => h('td', { class: 'number' }, String(row.counts[key]))),
              h('td', { class: 'number' }, String(row.counts.specEdits)),
              h('td', { class: 'number' }, String(row.counts.codeEdits)),
              ordered ? h('td', {}, orderMark(row.order)?.textContent ?? '') : null,
            ),
          ),
        ),
      ),
    ),
  );
}

/**
 * One bar of the share of the estimated context tokens each class brought in, named an estimate,
 * with its table; partial where a request lacked its counts.
 * @param {import('../model/consultation.js').Estimate | undefined} tokens
 * @param {boolean} partial
 * @param {HTMLElement} tip
 */
function estimate(tokens, partial, tip) {
  if (!tokens) return null;
  const sum = CLASSES.reduce((all, [key]) => all + tokens[key], 0);
  const parts = CLASSES.map(([key, words]) => ({
    key,
    words,
    value: tokens[key],
    said: `${words}: an estimated ${tokens[key].toLocaleString()} tokens (${percent(tokens[key], sum)})`,
  }));
  return h(
    'figure',
    { class: 'consult-chart consult-estimate' },
    h('figcaption', {}, h('h4', {}, 'Share of the context each class brought in, an estimate')),
    legend(CLASSES),
    sum ? h('div', { class: 'consult-rows' }, h('div', { class: 'consult-row' }, h('span', { class: 'consult-name' }, 'All agents'), bar(parts, sum, tip), h('span', { class: 'consult-total' }, `${sum.toLocaleString()} tokens`))) : h('p', { class: 'empty' }, 'No request has been estimated yet.'),
    partial ? h('p', { class: 'consult-partial' }, 'The estimate is partial: a request lacked its counts, or a file of another format was left out.') : null,
    h(
      'details',
      { class: 'consult-table' },
      h('summary', {}, 'The estimate, as a table'),
      h(
        'div',
        { class: 'table-wrap' },
        h(
          'table',
          { class: 'tokens-table' },
          h('thead', {}, h('tr', {}, h('td', {}), h('th', { scope: 'col', class: 'number' }, 'Tokens'), h('th', { scope: 'col', class: 'number' }, 'Share'))),
          h('tbody', {}, parts.map((part) => h('tr', {}, h('th', { scope: 'row' }, part.words), h('td', { class: 'number' }, part.value.toLocaleString()), h('td', { class: 'number' }, percent(part.value, sum))))),
        ),
      ),
    ),
  );
}

/**
 * The sessions whose adapter counted nothing, apart.
 * @param {{ session: string, host: string }[]} sessions
 */
function unmeasuredNote(sessions) {
  if (!sessions.length) return null;
  return h(
    'p',
    { class: 'consult-unmeasured' },
    `Unmeasured: ${count(sessions.length, 'session')} whose adapter does not count what its agents consult: `,
    sessions.map((each) => `${each.session} (${each.host})`).join(', '),
  );
}
