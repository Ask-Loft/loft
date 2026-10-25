// A `findings` view: the count of errors and of findings at each severity, each one with its
// rule, its message, where it is and the node it names, marked as introduced by the branch or
// there before it, the agent's reason beside a finding left in place, and those resolved.

import { h } from '../../lib/dom.js';
import * as kept from '../../lib/kept.js';
import { count, place } from '../../lib/format.js';
import { chip, leftOut, named } from './index.js';

const SEVERITIES = ['error', 'warning', 'info', 'hint'];

/**
 * @param {any} report
 * @param {any} body
 * @param {{ key?: string }} [context]
 */
export function findingsView(report, body, context = {}) {
  const path = context.key ? `${context.key}/resolved` : null;
  const check = report.check ?? {};
  const errors = check.errors ?? [];
  const findings = check.findings ?? [];
  const resolved = check.resolved ?? [];
  const reasons = new Map((body.reasons ?? []).map((reason) => [`${reason.rule} ${reason.key}`, reason.reason]));
  const base = check.baseline?.revision;
  const counts = SEVERITIES.map((severity) => ({
    severity,
    count: findings.filter((finding) => finding.severity === severity).length,
  }));
  if (errors.length === 0 && findings.length === 0) {
    return [
      h('p', { class: 'clean' }, 'The specification is clean: no error and no finding.'),
      base ? h('p', { class: 'base' }, `Marked against ${base}.`) : null,
      resolvedList(resolved, path),
    ];
  }
  return [
    h(
      'p',
      { class: 'counts' },
      errors.length ? chip(count(errors.length, 'error'), 'danger') : null,
      counts.filter((each) => each.count).map((each) => chip(`${each.count} ${each.severity}`, each.severity)),
    ),
    base ? h('p', { class: 'base' }, `Each finding is marked against ${base}.`) : null,
    errors.length
      ? h(
          'ul',
          { class: 'findings' },
          errors.map((error) =>
            h(
              'li',
              { class: 'finding' },
              h('span', { class: 'rule' }, error.kind),
              ' ',
              chip('error', 'danger'),
              ' ',
              error.message,
              (error.sites ?? []).map(site),
            ),
          ),
        )
      : null,
    h(
      'ul',
      { class: 'findings' },
      findings.map((finding) => {
        const subject = finding.subjects?.[0];
        const reason = subject ? reasons.get(`${finding.rule} ${subject.key}`) : undefined;
        const node = subject ? report.nodes?.[subject.key] : undefined;
        return h(
          'li',
          { class: 'finding' },
          h('span', { class: 'rule' }, finding.rule),
          ' ',
          chip(finding.severity, finding.severity),
          ' ',
          chip(finding.change === 'introduced' ? 'introduced' : 'there before', finding.change === 'introduced' ? 'warning' : 'muted'),
          h('p', {}, finding.message),
          site(finding),
          node ? h('p', { class: 'node' }, named(node)) : null,
          reason ? h('p', { class: 'reason' }, `Left in place: ${reason}`) : null,
        );
      }),
    ),
    leftOut(report, 'check.findings'),
    resolvedList(resolved, path),
  ];
}

/** @param {any} item */
function site(item) {
  if (!item.path) return null;
  return h('p', { class: 'site' }, h('code', {}, place(item)));
}

/**
 * @param {any[]} resolved
 * @param {string | null} path
 */
function resolvedList(resolved, path) {
  if (!resolved.length) return null;
  return kept.details(
    { path, open: false },
    { class: 'fold resolved' },
    h('summary', {}, `${resolved.length} resolved by the branch`),
    h('ul', { class: 'findings' }, resolved.map((finding) => h('li', {}, h('span', { class: 'rule' }, finding.rule), ' ', finding.message))),
  );
}
