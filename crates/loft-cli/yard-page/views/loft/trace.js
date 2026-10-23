// A `trace` view: how many of the branch's criteria are linked to tests and how many of those
// links are accepted, and, folded beneath, each criterion under its Feature with its tests, those
// that need a look first, and the findings `loft reconcile` reported.

import { h } from '../../lib/dom.js';
import { chip, leftOut, named } from './index.js';

/** Whether the person opened each `trace` view's list, by its block's key; folded unless they did. */
const folds = new Map();

const STATES = {
  accepted: { word: 'accepted', tone: 'success' },
  unreviewed: { word: 'not yet reviewed', tone: 'warning' },
  stale: { word: 'stale', tone: 'danger' },
  unverified: { word: 'unverified', tone: 'info' },
  unlinked: { word: 'not linked', tone: 'muted' },
};

/**
 * The state of each criterion's link: not linked, unverified with its reason, stale, not yet
 * reviewed, or accepted.
 * @param {any} criterion
 * @param {Map<string, string[]>} rules the rules reported of each node, by its key
 * @param {Map<string, any[]>} tests
 */
export function linkState(criterion, rules, tests) {
  const reported = rules.get(criterion.key) ?? [];
  if (reported.includes('TR-001')) return 'unlinked';
  if (criterion.unverified) return 'unverified';
  if (reported.some((rule) => ['TR-003', 'TR-006', 'TR-004', 'TR-009', 'TR-010'].includes(rule))) return 'stale';
  if (reported.includes('TR-002')) return 'unreviewed';
  if (!(tests.get(criterion.key) ?? []).length) return 'unlinked';
  return 'accepted';
}

/**
 * @param {any} report
 * @param {any} [_body]
 * @param {{ key?: string }} [context]
 */
export function traceView(report, _body, context = {}) {
  const reconcile = report.reconcile ?? {};
  /** @type {Map<string, string[]>} */
  const rules = new Map();
  for (const finding of reconcile.findings ?? []) {
    if (finding.key) rules.set(finding.key, [...(rules.get(finding.key) ?? []), finding.rule]);
  }
  /** @type {Map<string, any[]>} */
  const tests = new Map();
  for (const test of reconcile.tests ?? []) tests.set(test.key, [...(tests.get(test.key) ?? []), test]);
  const features = (report.features ?? []).map((feature) => ({
    ...feature,
    criteria: (feature.criteria ?? []).map((criterion) => ({ ...criterion, state: linkState(criterion, rules, tests) })),
  }));
  const criteria = features.flatMap((feature) => feature.criteria);
  const linked = criteria.filter((criterion) => criterion.state !== 'unlinked').length;
  const accepted = criteria.filter((criterion) => criterion.state === 'accepted').length;
  const states = Object.keys(STATES)
    .map((state) => ({ state, count: criteria.filter((criterion) => criterion.state === state).length }))
    .filter((each) => each.count);
  const share = (count) => (criteria.length ? `${(count / criteria.length) * 100}%` : '0%');
  return [
    h(
      'div',
      { class: 'coverage', role: 'img', 'aria-label': `${linked} of ${criteria.length} criteria linked, ${accepted} accepted` },
      h('span', { class: 'coverage-linked', style: `width:${share(linked)}` }),
      h('span', { class: 'coverage-accepted', style: `width:${share(accepted)}` }),
    ),
    h('p', { class: 'coverage-words' }, `${linked} of ${criteria.length} of the branch's criteria linked to tests, ${accepted} of those accepted.`),
    states.length > 1 ? h('p', { class: 'counts' }, states.map((each) => chip(`${each.count} ${STATES[each.state].word}`, STATES[each.state].tone))) : null,
    report.base ? h('p', { class: 'base' }, `Compared with ${report.base}.`) : null,
    h(
      'details',
      {
        class: ['fold', 'trace-list'],
        open: Boolean(context.key && folds.get(context.key)),
        ontoggle: (event) => {
          if (context.key) folds.set(context.key, /** @type {HTMLDetailsElement} */ (event.currentTarget).open);
        },
      },
      h('summary', {}, `${counted(criteria.length, 'criterion', 'criteria')} under ${counted(features.length, 'Feature', 'Features')}`),
      features.map((feature) => featureBlock(feature, tests)),
      leftOut(report, 'features'),
      findingsList(reconcile.findings ?? []),
    ),
  ];
}

/**
 * `count` with its noun, singular for one.
 * @param {number} count
 * @param {string} one
 * @param {string} many
 */
function counted(count, one, many) {
  return `${count} ${count === 1 ? one : many}`;
}

/**
 * The findings `loft reconcile` reported, folded into one line that counts them.
 * @param {any[]} findings
 */
function findingsList(findings) {
  return findings.length
    ? h(
        'details',
        { class: 'fold' },
        h('summary', {}, `${findings.length} findings of loft reconcile`),
        h('ul', { class: 'findings' }, findings.map((finding) => h('li', {}, h('span', { class: 'rule' }, finding.rule), ' ', finding.label ? `${finding.label}: ` : '', finding.message))),
      )
    : null;
}

/**
 * A Feature's criteria, the ones that need a look first, the accepted ones folded into one line.
 * @param {any} feature
 * @param {Map<string, any[]>} tests
 */
function featureBlock(feature, tests) {
  const open = feature.criteria.filter((criterion) => criterion.state !== 'accepted');
  const accepted = feature.criteria.filter((criterion) => criterion.state === 'accepted');
  const line = (criterion) =>
    h(
      'li',
      { class: ['criterion', `criterion--${criterion.state}`] },
      chip(STATES[criterion.state].word, STATES[criterion.state].tone),
      ' ',
      named(criterion),
      criterion.unverified ? h('p', { class: 'reason' }, criterion.unverified) : null,
      h(
        'ul',
        { class: 'tests' },
        (tests.get(criterion.key) ?? []).map((test) =>
          h('li', {}, h('code', {}, `${test.file}:${test.locations?.[0]?.firstLine ?? ''}`), ' ', test.name),
        ),
      ),
    );
  return h(
    'section',
    { class: 'trace-feature' },
    h('h5', {}, named(feature)),
    h('ul', { class: 'criteria' }, open.map(line)),
    accepted.length
      ? h('details', { class: 'fold' }, h('summary', {}, `${accepted.length} accepted`), h('ul', { class: 'criteria' }, accepted.map(line)))
      : null,
  );
}
