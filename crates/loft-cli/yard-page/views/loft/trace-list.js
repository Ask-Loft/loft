// The list of a `trace` view: each Feature folded to its label, its name, the number of its
// criteria linked and a bar; opened, one table of its criteria, a group of rows for each
// Requirement headed by its statement, the criteria that need a look first and the accepted ones
// behind one line. A report recorded before Requirements were given lists the criteria in one
// group.

import { h, inline } from '../../lib/dom.js';
import { chip, nameOf, named, tag } from './index.js';

export const STATES = {
  accepted: { word: 'accepted', tone: 'success' },
  unreviewed: { word: 'not yet reviewed', tone: 'warning' },
  stale: { word: 'stale', tone: 'danger' },
  unverified: { word: 'unverified', tone: 'info' },
  unlinked: { word: 'not linked', tone: 'muted' },
};

/** The word an EARS form opens with, and the word before its system, where it has them. */
const OPENINGS = {
  event_driven: ['When'],
  state_driven: ['While'],
  optional: ['Where'],
  unwanted: ['If', 'then'],
};

/**
 * What the person opened, by the key of what they opened; nothing is remembered without a key.
 * @typedef {{ open: (key: string | null) => boolean, set: (key: string | null, open: boolean) => void }} Folds
 */

/**
 * A bar of the share of `criteria` linked and accepted.
 * @param {any[]} criteria
 * @param {string} [className]
 */
export function coverage(criteria, className = 'coverage') {
  const linked = criteria.filter((criterion) => criterion.state !== 'unlinked').length;
  const accepted = criteria.filter((criterion) => criterion.state === 'accepted').length;
  const share = (count) => (criteria.length ? `${(count / criteria.length) * 100}%` : '0%');
  return h(
    'span',
    { class: className, role: 'img', 'aria-label': `${linked} of ${criteria.length} criteria linked, ${accepted} accepted` },
    h('span', { class: 'coverage-linked', style: `width:${share(linked)}` }),
    h('span', { class: 'coverage-accepted', style: `width:${share(accepted)}` }),
  );
}

/**
 * A Feature, folded to its label, its name, the number of its criteria linked and a bar.
 * @param {any} feature its criteria each with its state
 * @param {Map<string, any[]>} tests
 * @param {Folds} folds
 * @param {string | null} key the key its fold is remembered by
 */
export function featureFold(feature, tests, folds, key) {
  const linked = feature.criteria.filter((criterion) => criterion.state !== 'unlinked').length;
  return h(
    'details',
    { class: 'trace-feature', open: folds.open(key), ontoggle: (event) => folds.set(key, /** @type {HTMLDetailsElement} */ (event.currentTarget).open) },
    h(
      'summary',
      {},
      h('span', { class: 'feature-name' }, named(feature)),
      h('span', { class: 'feature-count' }, `${linked} of ${feature.criteria.length} linked`),
      coverage(feature.criteria, 'coverage coverage--small'),
    ),
    criteriaTable(groups(feature), tests, folds, key),
  );
}

/**
 * A Feature's criteria in groups: one for each Requirement, in the Requirements' order, or one
 * with no Requirement where the report gives none.
 * @param {any} feature
 * @returns {{ requirement: any, criteria: any[] }[]}
 */
function groups(feature) {
  if (!feature.requirements?.length) return [{ requirement: null, criteria: feature.criteria }];
  return feature.requirements.map((requirement) => ({
    requirement,
    criteria: feature.criteria.filter((criterion) => criterion.requirement === requirement.key),
  }));
}

/**
 * One table of a Feature's criteria, a group of rows for each Requirement.
 * @param {{ requirement: any, criteria: any[] }[]} groups
 * @param {Map<string, any[]>} tests
 * @param {Folds} folds
 * @param {string | null} key
 */
function criteriaTable(groups, tests, folds, key) {
  return h(
    'div',
    { class: 'table-wrap' },
    h(
      'table',
      { class: 'criteria-table' },
      h('thead', {}, h('tr', {}, ['Label', 'Criterion', 'State', 'Tests'].map((word) => h('th', { scope: 'col' }, word)))),
      groups.map((group) => criteriaGroup(group, tests, folds, key && `${key}/${group.requirement?.key ?? 'all'}`)),
    ),
  );
}

/**
 * A Requirement's rows, headed by its statement: the criteria that need a look, then a line that
 * opens the accepted ones.
 * @param {{ requirement: any, criteria: any[] }} group
 * @param {Map<string, any[]>} tests
 * @param {Folds} folds
 * @param {string | null} key
 */
function criteriaGroup({ requirement, criteria }, tests, folds, key) {
  const open = criteria.filter((criterion) => criterion.state !== 'accepted');
  const accepted = criteria.filter((criterion) => criterion.state === 'accepted');
  const shown = folds.open(key);
  const hidden = h('tbody', { class: 'accepted', hidden: !shown }, accepted.map((criterion) => row(criterion, tests)));
  const toggle = h(
    'button',
    {
      type: 'button',
      class: 'accepted-toggle',
      'aria-expanded': String(shown),
      onclick: (event) => {
        const button = /** @type {HTMLButtonElement} */ (event.currentTarget);
        const now = button.getAttribute('aria-expanded') !== 'true';
        button.setAttribute('aria-expanded', String(now));
        hidden.hidden = !now;
        folds.set(key, now);
      },
    },
    `${accepted.length} accepted`,
  );
  return [
    h(
      'tbody',
      { class: 'requirement' },
      requirement
        ? h('tr', { class: 'requirement-row' }, h('th', { scope: 'rowgroup', colspan: '4' }, requirement.label ? tag(requirement.label) : null, ' ', statement(requirement)))
        : null,
      open.map((criterion) => row(criterion, tests)),
      accepted.length ? h('tr', { class: 'accepted-row' }, h('td', { colspan: '4' }, toggle)) : null,
    ),
    accepted.length ? hidden : null,
  ];
}

/**
 * A Requirement's statement: in an EARS form, its keywords set apart from its slots, each slot
 * drawn from the HTML `loft` rendered from its Markdown; otherwise its sentence.
 * @param {any} requirement
 */
function statement(requirement) {
  const parts = requirement.statement;
  if (!parts) return h('span', { class: 'statement' }, nameOf(requirement));
  const slot = (name) => (parts[`${name}Html`] ? inline(parts[`${name}Html`]) : parts[name] ?? '');
  const word = (text) => h('span', { class: 'ears-word' }, text);
  const [opening, then] = OPENINGS[parts.form] ?? [];
  return h(
    'span',
    { class: 'statement' },
    opening ? [word(opening), ' ', slot('condition'), ', '] : null,
    then ? [word(then), ' '] : null,
    slot('system'),
    ' ',
    word('shall'),
    ' ',
    slot('shall'),
  );
}

/**
 * A criterion's row: its label, or `new` before it has one; its name, with the reason it is
 * unverified; its state; and its tests by file and line.
 * @param {any} criterion
 * @param {Map<string, any[]>} tests
 */
function row(criterion, tests) {
  const state = STATES[criterion.state];
  return h(
    'tr',
    { class: ['criterion', `criterion--${criterion.state}`] },
    h('td', { class: 'criterion-label' }, criterion.label ? tag(criterion.label) : h('span', { class: ['label', 'label--new'] }, 'new')),
    h('td', { class: 'criterion-name' }, nameOf(criterion), criterion.unverified ? h('p', { class: 'reason' }, criterion.unverified) : null),
    h('td', { class: 'criterion-state' }, chip(state.word, state.tone)),
    h(
      'td',
      { class: 'criterion-tests' },
      (tests.get(criterion.key) ?? []).map((test) => {
        const line = test.locations?.[0]?.firstLine;
        return h('div', { class: 'test' }, h('code', {}, line ? `${test.file}:${line}` : test.file), h('span', { class: 'test-name' }, test.name));
      }),
    ),
  );
}
