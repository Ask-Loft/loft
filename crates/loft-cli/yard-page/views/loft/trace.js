// A `trace` view: how many of the branch's criteria are linked to tests and how many of those
// links are accepted, and, folded beneath, each Feature with its criteria and the findings
// `loft reconcile` reported.

import { h } from '../../lib/dom.js';
import { chip, leftOut } from './index.js';
import { STATES, coverage, featureFold } from './trace-list.js';

/** What the person opened in each `trace` view, by its block's key and what they opened in it. */
const opened = new Map();

/** @type {import('./trace-list.js').Folds} */
const folds = {
  open: (key) => Boolean(key && opened.get(key)),
  set: (key, open) => {
    if (key) opened.set(key, open);
  },
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
  const key = context.key ?? null;
  return [
    coverage(criteria),
    h('p', { class: 'coverage-words' }, `${linked} of ${criteria.length} of the branch's criteria linked to tests, ${accepted} of those accepted.`),
    states.length > 1 ? h('p', { class: 'counts' }, states.map((each) => chip(`${each.count} ${STATES[each.state].word}`, STATES[each.state].tone))) : null,
    report.base ? h('p', { class: 'base' }, `Compared with ${report.base}.`) : null,
    h(
      'details',
      { class: ['fold', 'trace-list'], open: folds.open(key), ontoggle: (event) => folds.set(key, /** @type {HTMLDetailsElement} */ (event.currentTarget).open) },
      h('summary', {}, `${counted(criteria.length, 'criterion', 'criteria')} under ${counted(features.length, 'Feature', 'Features')}`),
      features.map((feature) => featureFold(feature, tests, folds, key && `${key}/${feature.key}`)),
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
