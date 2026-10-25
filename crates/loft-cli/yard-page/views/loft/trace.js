// A `trace` view: how many of the branch's criteria are linked to tests and how many of those
// links are accepted, and, folded beneath, each Feature with its criteria and the rules
// `loft reconcile` reported of them.

import { h } from '../../lib/dom.js';
import { count } from '../../lib/format.js';
import * as kept from '../../lib/kept.js';
import { chip, leftOut } from './index.js';
import { STATES, coverage, featureFold } from './trace-list.js';

/**
 * The state of each criterion's link: not linked, unverified with its reason, stale, not yet
 * reviewed, or accepted. It is taken from the tests and rules recorded with the criterion, which
 * no shortened list leaves out, and from the report's lists for a view recorded without them.
 * @param {any} criterion
 * @param {Map<string, string[]>} rules the rules reported of each node, by its key
 * @param {Map<string, any[]>} tests
 */
export function linkState(criterion, rules, tests) {
  const own = Array.isArray(criterion.rules) && Array.isArray(criterion.tests);
  const reported = own ? criterion.rules : rules.get(criterion.key) ?? [];
  const listed = own ? criterion.tests : tests.get(criterion.key) ?? [];
  if (reported.includes('TR-001')) return 'unlinked';
  if (criterion.unverified) return 'unverified';
  if (reported.some((rule) => ['TR-003', 'TR-006', 'TR-004', 'TR-009', 'TR-010'].includes(rule))) return 'stale';
  if (reported.includes('TR-002')) return 'unreviewed';
  if (!listed.length) return 'unlinked';
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
  const key = context.key ? `${context.key}/trace` : null;
  return [
    coverage(criteria),
    h('p', { class: 'coverage-words' }, `${linked} of ${criteria.length} of the branch's criteria linked to tests, ${accepted} of those accepted.`),
    states.length > 1 ? h('p', { class: 'counts' }, states.map((each) => chip(`${each.count} ${STATES[each.state].word}`, STATES[each.state].tone))) : null,
    short(report, criteria) ? h('p', { class: 'short' }, 'Recorded by an older loft: this view may count fewer links than loft reconcile found.') : null,
    report.base ? h('p', { class: 'base' }, `Compared with ${report.base}.`) : null,
    kept.details(
      { path: key, open: false },
      { class: ['fold', 'trace-list'] },
      h('summary', {}, `${count(criteria.length, 'criterion', 'criteria')} under ${count(features.length, 'Feature')}`),
      features.map((feature) => featureFold(feature, tests, kept.folds, key && `${key}/${feature.key}`)),
      leftOut(report, 'features'),
      findingsList(findingsOf(criteria, reconcile), context.key ? `${context.key}/findings` : null),
    ),
  ];
}

/**
 * Whether a view recorded without each criterion's tests and rules had its report's lists of
 * tests or findings shortened, so that its counts may fall short.
 * @param {any} report
 * @param {any[]} criteria
 */
function short(report, criteria) {
  const left = report.leftOut ?? {};
  return !criteria.some((criterion) => Array.isArray(criterion.tests)) && Boolean(left['reconcile.tests'] || left['reconcile.findings']);
}

/**
 * The findings to list: each rule `loft reconcile` reported of one of the branch's criteria, with
 * the criterion's label and name; or, for a view recorded without each criterion's rules, the
 * findings of the report with their messages.
 * @param {any[]} criteria
 * @param {any} reconcile
 * @returns {{ rule: string, label?: string | null, message: string }[]}
 */
function findingsOf(criteria, reconcile) {
  if (!criteria.some((criterion) => Array.isArray(criterion.rules))) return reconcile.findings ?? [];
  return criteria.flatMap((criterion) => (criterion.rules ?? []).map((rule) => ({ rule, label: criterion.label, message: criterion.name })));
}

/**
 * The findings, folded into one line that counts them.
 * @param {{ rule: string, label?: string | null, message: string }[]} findings
 * @param {string | null} path where the fold is kept
 */
function findingsList(findings, path) {
  return findings.length
    ? kept.details(
        { path, open: false },
        { class: 'fold' },
        h('summary', {}, `${findings.length} findings of loft reconcile`),
        h('ul', { class: 'findings' }, findings.map((finding) => h('li', {}, h('span', { class: 'rule' }, finding.rule), ' ', finding.label ? `${finding.label}: ` : '', finding.message))),
      )
    : null;
}
