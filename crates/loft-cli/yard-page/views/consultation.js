// What the agents consulted, as the dataviz method draws it with no library: a legend of the
// classes in one row, then a chart of stacked bars of each stage's reads in Yard Log's order, each
// marked with whether the specification was consulted before the code was first edited; the same
// bars for the main agent and each type of subagent; and one bar of the share of the estimated
// context tokens each class brought in. Each class keeps one hue in every chart and both themes,
// the specification's the most prominent. Each segment names its class, count and share when
// pointed at or focused, and each chart has a table of its counts beneath it. The page sums
// nothing: it draws the sums the page server gives.

import { h, svg } from '../lib/dom.js';
import { count, tokens as amount } from '../lib/format.js';
import { CLASSES, READS, order, reads, summary } from '../model/consultation.js';

/** @typedef {import('../model/consultation.js').Counts} Counts */
/** @typedef {{ key: string, words: string, value: number, said: string }} Part */
/** @typedef {{ name: string, counts: Counts, order?: any }} Row */

/**
 * The consultation view of `log`.
 * @param {any} log
 */
export function consultationView(log) {
  const sums = summary(log.consultationSums);
  const section = h('section', { class: 'consultation', 'aria-label': 'What the agents consulted' });
  if (!sums.measured) {
    section.append(h('p', { class: 'empty' }, 'Nothing has been measured yet: no session’s adapter counted what its agents consulted.'));
    const unmeasured = unmeasuredNote(sums.unmeasured);
    if (unmeasured) section.append(unmeasured);
    return section;
  }
  // Each mark is named for a reader by its own label; the tip shows the same words to the eye.
  const tip = h('div', { class: 'consult-tip', 'aria-hidden': 'true', hidden: true });
  /** @type {Row[]} */
  const stages = sums.stages.map((stage) => ({ name: stage.title || stage.stage, counts: stage.counts, order: stage }));
  if (sums.noStage) stages.push({ name: 'No stage', counts: sums.noStage.counts, order: sums.noStage });
  const agents = sums.agents.map((agent) => ({ name: agent.type === 'main' ? 'main agent' : `${agent.type} subagent`, counts: agent.counts }));
  const show = (/** @type {Event} */ event) => point(tip, /** @type {Element} */ (event.target));
  section.addEventListener('pointerover', show);
  section.addEventListener('focusin', show);
  section.addEventListener('pointerout', () => point(tip, null));
  section.addEventListener('focusout', () => point(tip, null));
  const parts = [legend(READS), chart('By stage', stages), chart('By agent', agents), estimate(sums.all?.estimate, sums.partial), unmeasuredNote(sums.unmeasured), tip];
  for (const part of parts) if (part) section.append(part);
  return section;
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
 * Shows the label of `mark` in the tip beside it, or hides the tip where `mark` is no mark.
 * @param {HTMLElement} tip
 * @param {Element | null} mark
 */
function point(tip, mark) {
  const said = mark?.classList.contains('consult-mark') ? mark.getAttribute('aria-label') : null;
  tip.hidden = !said;
  tip.textContent = said ?? '';
  if (!said || !mark) return;
  const view = tip.parentElement?.getBoundingClientRect();
  const at = mark.getBoundingClientRect();
  tip.style.left = `${Math.max(0, at.left - (view?.left ?? 0))}px`;
  tip.style.top = `${at.bottom - (view?.top ?? 0) + 4}px`;
}

/**
 * One bar of `parts`, each a segment as long as its share of `scale`, a stroke of the surface
 * leaving a gap between them; each can be focused and is named by its words.
 * @param {Part[]} parts
 * @param {number} scale
 */
function bar(parts, scale) {
  let x = 0;
  const segments = parts
    .filter((part) => part.value > 0)
    .map((part) => {
      const width = (part.value / scale) * 100;
      const segment = svg('rect', { class: `consult-mark consult--${part.key}`, x: `${x}%`, y: 0, width: `${width}%`, height: '100%', rx: 2, tabindex: 0, role: 'img', 'aria-label': part.said });
      x += width;
      return segment;
    });
  return svg('svg', { class: 'consult-bar', width: '100%', height: 16, 'aria-hidden': segments.length ? null : 'true' }, segments);
}

/** @param {number} part @param {number} whole */
function percent(part, whole) {
  return whole ? `${Math.round((part / whole) * 100)}%` : '0%';
}

/**
 * The mark of whether a stage's specification was consulted before its code was first edited.
 * @param {any} scope
 */
function orderMark(scope) {
  const first = order(scope);
  return first ? h('span', { class: `consult-line--${first.kind}` }, `${first.glyph} ${first.words}`) : null;
}

/**
 * A chart of stacked bars, a row to each of `rows`, all on one scale, with its table beneath.
 * @param {string} title
 * @param {Row[]} rows
 */
function chart(title, rows) {
  const totals = rows.map((row) => reads(row.counts));
  const scale = Math.max(1, ...totals);
  const columns = [...READS.map(([, words]) => words), 'spec edits', 'code edits'];
  const ordered = rows.some((row) => row.order);
  return h(
    'figure',
    { class: 'consult-chart' },
    h('figcaption', {}, h('h4', {}, title)),
    rows.length ? null : h('p', { class: 'empty' }, 'No call has been counted here yet.'),
    h(
      'div',
      { class: 'consult-rows' },
      rows.map((row, index) => {
        const sum = totals[index];
        const parts = READS.map(([key, words]) => ({ key, words, value: row.counts[key], said: `${words}: ${row.counts[key]} of ${sum} (${percent(row.counts[key], sum)})` }));
        return h(
          'div',
          { class: 'consult-row' },
          h('span', { class: 'consult-name', title: row.name }, row.name),
          bar(parts, scale),
          h('span', { class: 'consult-total' }, String(sum), row.order ? orderMark(row.order) : null),
        );
      }),
    ),
    table(
      `${title}, as a table`,
      ordered ? [...columns, 'Order'] : columns,
      rows.map((row) => {
        const counts = [...READS.map(([key]) => row.counts[key]), row.counts.specEdits, row.counts.codeEdits].map(String);
        return [row.name, ...counts, ...(ordered ? [order(row.order)?.words ?? ''] : [])];
      }),
    ),
  );
}

/**
 * A table of `rows` under `columns`, folded beneath its chart: the chart's text alternative.
 * @param {string} caption
 * @param {string[]} columns
 * @param {string[][]} rows each a row's name, then its cells
 */
function table(caption, columns, rows) {
  return h(
    'details',
    { class: 'consult-table' },
    h('summary', {}, caption),
    h(
      'div',
      { class: 'table-wrap' },
      h(
        'table',
        { class: 'tokens-table' },
        h('thead', {}, h('tr', {}, h('td', {}), columns.map((column) => h('th', { scope: 'col', class: 'number' }, column)))),
        h('tbody', {}, rows.map(([name, ...cells]) => h('tr', {}, h('th', { scope: 'row' }, name), cells.map((cell) => h('td', { class: 'number' }, cell))))),
      ),
    ),
  );
}

/**
 * One bar of the share of the estimated context tokens each class brought in, named an estimate,
 * with its table; partial where a request lacked its counts.
 * @param {import('../model/consultation.js').Estimate | undefined} tokens
 * @param {boolean} partial
 */
function estimate(tokens, partial) {
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
    sum
      ? h(
          'div',
          { class: 'consult-rows' },
          h(
            'div',
            { class: 'consult-row' },
            h('span', { class: 'consult-name' }, 'All agents'),
            bar(parts, sum),
            h('span', { class: 'consult-total', title: `${sum.toLocaleString()} tokens` }, `${amount(sum)} tokens`),
          ),
        )
      : h('p', { class: 'empty' }, 'No request has been estimated yet.'),
    partial ? h('p', { class: 'consult-partial' }, 'The estimate is partial: a request lacked its counts, or a file of another format was left out.') : null,
    table(
      'The estimate, as a table',
      ['Tokens', 'Share'],
      parts.map((part) => [part.words, part.value.toLocaleString(), percent(part.value, sum)]),
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
