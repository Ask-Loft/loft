// A `decision` view: the decision's title, label and state, its context and outcome, its
// options with the selected one marked, a compact table of its assessments, each cell a rating
// as a symbol and a word whose rationale shows on hover or focus and in the option's row opened,
// the selected option's row first; its consequences under a heading for each kind, and its
// subjects one line for each role.

import { h } from '../../lib/dom.js';
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
    decision.context ? h('p', { class: 'context' }, decision.context) : null,
    state.outcome ? h('p', { class: 'outcome' }, h('strong', {}, 'Outcome: '), state.outcome) : null,
    state.rationale ? h('p', { class: 'rationale' }, h('strong', {}, 'Rationale: '), state.rationale) : null,
    h(
      'ul',
      { class: 'decision-options' },
      options.map((option) =>
        h(
          'li',
          { class: ['decision-option', option.selected ? 'is-selected' : null] },
          h('strong', {}, option.title),
          option.selected ? h('span', { class: 'chosen-word' }, 'Selected') : null,
          option.description ? h('p', {}, option.description) : null,
        ),
      ),
    ),
    drivers.length && options.length ? matrix(options, drivers, rating) : null,
    drivers.length
      ? kept.details(
          { path: context.key ? `${context.key}/drivers` : null, open: false },
          { class: 'fold' },
          h('summary', {}, `${drivers.length} drivers`),
          h('ul', {}, drivers.map((driver) => h('li', {}, h('strong', {}, driver.label ?? driver.kind), ' ', driver.statement ?? (driver.node ? named(driver.node) : '')))),
        )
      : null,
    consequencesOf(decision.consequences ?? []),
    subjectsOf(decision.subjects ?? []),
  ];
}

/** Each rating's symbol, beside its word. */
const SYMBOLS = { supports: '✓', neutral: '–', conflicts: '✕' };

/** The heading of each kind of consequence, and the word that heads each role of subject. */
const KIND_HEADINGS = { benefit: 'Benefits', cost: 'Costs', obligation: 'Obligations', risk: 'Risks' };
const ROLE_WORDS = { affected: 'Affects', outcome: 'Outcome', supersedes: 'Supersedes' };

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
        option.description ? h('p', { class: 'option-description' }, option.description) : null,
        h(
          'ul',
          {},
          cells.map(([driver, cell]) => (cell ? h('li', {}, h('strong', {}, driverName(driver)), ' ', ratingMark(cell.rating), cell.rationale ? ` ${cell.rationale}` : '') : null)),
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
        },
      },
      option.title,
    );
    return [
      h(
        'tr',
        { class: option.selected ? 'is-chosen' : null },
        h('th', { scope: 'row' }, toggle),
        cells.map(([, cell]) =>
          h(
            'td',
            { class: cell ? `rating--${cell.rating}` : null, tabindex: cell?.rationale ? 0 : null },
            cell ? ratingMark(cell.rating) : '',
            cell?.rationale ? h('span', { class: 'rating-why', role: 'tooltip' }, cell.rationale) : null,
          ),
        ),
      ),
      opened,
    ];
  });
  return h(
    'div',
    { class: 'table-wrap' },
    h(
      'table',
      { class: 'assessments', onpointerover: place, onfocusin: place },
      h('thead', {}, h('tr', {}, h('th', { scope: 'col' }, 'Option'), drivers.map((driver) => h('th', { scope: 'col', title: driver.statement ?? '' }, driverName(driver))))),
      h('tbody', {}, rows),
    ),
  );
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
 * A rating as its symbol and its word, in its colour.
 * @param {string} rating
 */
function ratingMark(rating) {
  return h('span', { class: ['rating', `rating-mark--${rating}`] }, h('span', { 'aria-hidden': 'true' }, SYMBOLS[/** @type {keyof typeof SYMBOLS} */ (rating)] ?? '·'), ' ', rating);
}

/**
 * The consequences under a heading for each of their kinds.
 * @param {any[]} consequences
 */
function consequencesOf(consequences) {
  if (!consequences.length) return null;
  const kinds = [...new Set(consequences.map((consequence) => consequence.kind))];
  return h(
    'div',
    { class: 'consequences' },
    kinds.map((kind) =>
      h(
        'section',
        { class: 'consequence-kind', dataset: { kind } },
        h('p', { class: 'consequence-heading' }, KIND_HEADINGS[/** @type {keyof typeof KIND_HEADINGS} */ (kind)] ?? capital(kind)),
        h('ul', {}, consequences.filter((consequence) => consequence.kind === kind).map((consequence) => h('li', {}, consequence.statement))),
      ),
    ),
  );
}

/**
 * The subjects, one line for each role, headed by the role.
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
        'p',
        { class: 'subject-line', dataset: { role } },
        h('span', { class: 'subject-role' }, `${ROLE_WORDS[/** @type {keyof typeof ROLE_WORDS} */ (role)] ?? capital(role)}:`),
        ' ',
        subjects
          .filter((subject) => subject.role === role)
          .flatMap((subject, index) => [index ? h('span', { class: 'subject-sep', 'aria-hidden': 'true' }, ' · ') : null, h('span', { class: 'subject' }, subject.node ? named(subject.node) : subject.former ?? '')]),
      ),
    ),
  );
}

/** @param {string} word */
function capital(word) {
  return word ? word[0].toUpperCase() + word.slice(1) : word;
}
