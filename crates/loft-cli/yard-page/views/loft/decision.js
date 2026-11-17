// A `decision` view: the decision's title, label and state, then its outcome; its options once,
// as the rows of a compact table of its assessments, the selected one first and marked, each
// cell a rating as a symbol in its color whose rationale shows on hover or focus, each row
// opening to the option's description and every rationale, a line above the table naming the
// symbols; its consequences as one list, each led by its kind; its subjects one line for each
// role; and its context and drivers each folded.

import { h } from '../../lib/dom.js';
import { count } from '../../lib/format.js';
import * as kept from '../../lib/kept.js';
import { chip, named, tag } from './index.js';

const STATES = {
  accepted: 'success',
  proposed: 'info',
  rejected: 'danger',
  retired: 'muted',
  superseded: 'muted',
};

/**
 * @param {any} report
 * @param {any} [_body]
 * @param {{ key?: string }} [context]
 */
export function decisionView(report, _body, context = {}) {
  const decision = report.decision ?? {};
  const state = decision.state ?? {};
  const options = decision.options ?? [];
  const drivers = decision.drivers ?? [];
  const rating = new Map(
    (decision.assessments ?? []).map((assessment) => [`${assessment.option} ${assessment.driver}`, assessment]),
  );
  return [
    h(
      'header',
      { class: 'decision-view-head' },
      h('h5', {}, decision.label ? tag(decision.label) : null, ' ', decision.title),
      chip(state.kind ?? 'unknown', STATES[state.kind] ?? 'muted'),
      decision.label ? null : h('span', { class: 'unpublished' }, 'Not published yet: it has no label.'),
    ),
    state.outcome ? h('p', { class: 'outcome' }, h('strong', {}, 'Outcome: '), state.outcome) : null,
    state.rationale ? h('p', { class: 'rationale' }, h('strong', {}, 'Rationale: '), state.rationale) : null,
    options.length ? matrix(options, drivers, rating) : null,
    consequencesOf(decision.consequences ?? []),
    subjectsOf(decision.subjects ?? []),
    decision.context
      ? kept.details(
          { path: context.key ? `${context.key}/context` : null, open: false },
          { class: 'fold decision-context' },
          h('summary', {}, 'Context'),
          h('p', { class: 'context' }, decision.context),
        )
      : null,
    drivers.length
      ? kept.details(
          { path: context.key ? `${context.key}/drivers` : null, open: false },
          { class: 'fold' },
          h('summary', {}, count(drivers.length, 'driver')),
          h('ul', {}, drivers.map((driver) => h('li', {}, h('strong', {}, driver.label ?? driver.kind), ' ', driver.statement ?? (driver.node ? named(driver.node) : '')))),
        )
      : null,
  ];
}

/** Each rating's symbol, beside its word. */
const SYMBOLS = { supports: '✓', neutral: '–', conflicts: '✕' };

/** The word that heads each role of subject. */
const ROLE_WORDS = { affected: 'Affects', outcome: 'Stated as', supersedes: 'Supersedes' };

/**
 * The table of assessments: a row for each option, the selected one first, and a column for each
 * driver; each cell its rating, its rationale shown on hover or focus; each row opens to the
 * rationale of each of its ratings.
 * @param {any[]} options
 * @param {any[]} drivers
 * @param {Map<string, any>} rating
 */
function matrix(options, drivers, rating) {
  const ordered = [...options.filter((option) => option.selected), ...options.filter((option) => !option.selected)];
  const driverName = (/** @type {any} */ driver) => driver.label ?? (driver.node ? named(driver.node) : driver.kind);
  const rows = ordered.flatMap((option, index) => {
    const why = `why-${option.key ?? index}`;
    const cells = drivers.map((driver) => [driver, rating.get(`${option.key} ${driver.key}`)]);
    const opened = h(
      'tr',
      { class: 'option-why', id: why, hidden: true },
      h(
        'td',
        { colspan: drivers.length + 1 },
        // Held at the scroll box's left edge and no wider than it, whatever the table's width.
        h(
          'div',
          { class: 'option-why-body' },
          option.description ? h('p', { class: 'option-description' }, option.description) : null,
          h(
            'dl',
            { class: 'option-ratings' },
            cells.flatMap(([driver, cell]) =>
              cell ? [h('dt', {}, driverName(driver)), h('dd', { class: 'option-rating' }, ratingMark(cell.rating)), h('dd', { class: 'option-rationale' }, cell.rationale ?? '')] : [],
            ),
          ),
        ),
      ),
    );
    const toggle = h(
      'button',
      {
        type: 'button',
        class: 'option-open',
        'aria-expanded': 'false',
        'aria-controls': why,
        onclick: (/** @type {Event} */ event) => {
          const button = /** @type {HTMLElement} */ (event.currentTarget);
          const open = button.getAttribute('aria-expanded') !== 'true';
          button.setAttribute('aria-expanded', String(open));
          opened.hidden = !open;
          // The row's words keep within the width the table is seen in.
          const room = button.closest('.table-wrap')?.clientWidth;
          if (room) opened.style.setProperty('--room', `${room - 32}px`);
        },
      },
      option.title,
    );
    return [
      h(
        'tr',
        { class: option.selected ? 'is-chosen' : null },
        h('th', { scope: 'row' }, toggle, option.selected ? h('span', { class: 'sr-only' }, ' (selected)') : null),
        cells.map(([, cell]) =>
          h(
            'td',
            { class: cell ? `rating--${cell.rating}` : null, tabindex: cell?.rationale ? 0 : null },
            cell ? ratingMark(cell.rating, false) : '',
            cell?.rationale ? h('span', { class: 'rating-why', role: 'tooltip' }, cell.rationale) : null,
          ),
        ),
      ),
      opened,
    ];
  });
  const key = h(
    'p',
    { class: 'rating-key' },
    Object.keys(SYMBOLS).flatMap((each, index) => [index ? ' ' : null, ratingMark(each)]),
  );
  const table = h(
    'table',
    { class: 'assessments', onpointerover: place, onfocusin: place },
    h('thead', {}, h('tr', {}, h('th', { scope: 'col' }, 'Option'), drivers.map((driver) => h('th', { scope: 'col', title: driver.statement ?? '' }, driverName(driver))))),
    h('tbody', {}, rows),
  );
  return h('div', { class: 'decision-matrix' }, drivers.length ? key : null, h('div', { class: 'table-wrap' }, table));
}

/**
 * Places the rationale of the cell under the pointer or the focus beneath the cell, or above it
 * near the window's foot; it is fixed to the window, so the table's sideways scroll clips none of it.
 * @param {Event} event
 */
function place(event) {
  const cell = /** @type {HTMLElement | null} */ (/** @type {HTMLElement} */ (event.target).closest?.('td'));
  const why = /** @type {HTMLElement | null} */ (cell?.querySelector('.rating-why') ?? null);
  if (!cell || !why) return;
  const box = cell.getBoundingClientRect();
  why.style.left = `${Math.max(8, Math.min(box.left, window.innerWidth - 300))}px`;
  const below = box.bottom + 160 < window.innerHeight;
  why.style.top = below ? `${box.bottom + 4}px` : '';
  why.style.bottom = below ? '' : `${window.innerHeight - box.top + 4}px`;
}

/**
 * A rating as its symbol in its color, and its word, shown or only named to a reader.
 * @param {string} rating
 * @param {boolean} [word] whether the word is shown beside the symbol
 */
function ratingMark(rating, word = true) {
  return h(
    'span',
    { class: ['rating', `rating-mark--${rating}`] },
    h('span', { class: 'rating-symbol', 'aria-hidden': 'true' }, SYMBOLS[/** @type {keyof typeof SYMBOLS} */ (rating)] ?? '·'),
    h('span', { class: word ? null : 'sr-only' }, ` ${rating}`),
  );
}

/**
 * The consequences as one list without bullets, each led by its kind in a small muted word.
 * @param {any[]} consequences
 */
function consequencesOf(consequences) {
  if (!consequences.length) return null;
  return h(
    'ul',
    { class: 'consequences' },
    consequences.map((consequence) =>
      h('li', { class: 'consequence', dataset: { kind: consequence.kind } }, h('span', { class: 'consequence-kind' }, consequence.kind), h('span', {}, consequence.statement)),
    ),
  );
}

/**
 * The subjects, one to a line beside their role, in the column that gives the consequences'
 * kinds; each kept to one line, its name whole on hover.
 * @param {any[]} subjects
 */
function subjectsOf(subjects) {
  if (!subjects.length) return null;
  const roles = [...new Set(subjects.map((subject) => subject.role))];
  return h(
    'div',
    { class: 'subjects' },
    roles.map((role) =>
      h(
        'div',
        { class: 'subject-role-row', dataset: { role } },
        h('span', { class: 'subject-role' }, ROLE_WORDS[/** @type {keyof typeof ROLE_WORDS} */ (role)] ?? capital(role)),
        h(
          'ul',
          { class: 'subject-list' },
          subjects
            .filter((subject) => subject.role === role)
            .map((subject) => {
              const whole = subject.node ? [subject.node.label, subject.node.name].filter(Boolean).join(' ') : subject.former ?? '';
              return h('li', { class: 'subject', title: whole || null }, subject.node ? named(subject.node) : subject.former ?? '');
            }),
        ),
      ),
    ),
  );
}

/** @param {string} word */
function capital(word) {
  return word ? word[0].toUpperCase() + word.slice(1) : word;
}
