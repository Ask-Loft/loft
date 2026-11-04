// A `findings` view: the count of errors and of findings at each severity, each error with its
// rule, its message and where it is; the findings grouped by rule, the most severe first, each
// group giving its rule, its severity and its message once and counting the findings the branch
// introduced and those there before it, open where the branch introduced one; within a group,
// each finding on one line with the node it names and where it is, those introduced first, and
// the agent's reason beside a finding left in place; and the findings the branch resolved.

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
    h('p', { class: 'finding-split' }, split(findings)),
    byRule(findings).map((group) => ruleGroup(group, report, reasons, context.key)),
    leftOut(report, 'check.findings'),
    resolvedList(resolved, path),
  ];
}

/**
 * The findings of each rule, most severe first, each rule's split into those the branch
 * introduced and those there before it.
 * @param {any[]} findings
 */
export function byRule(findings) {
  /** @type {Map<string, { rule: string, severity: string, message: string, introduced: any[], before: any[] }>} */
  const groups = new Map();
  for (const finding of findings) {
    const group = groups.get(finding.rule) ?? { rule: finding.rule, severity: finding.severity, message: finding.message, introduced: [], before: [] };
    if (SEVERITIES.indexOf(finding.severity) < SEVERITIES.indexOf(group.severity)) group.severity = finding.severity;
    (finding.change === 'introduced' ? group.introduced : group.before).push(finding);
    groups.set(finding.rule, group);
  }
  const rank = (/** @type {string} */ severity) => {
    const at = SEVERITIES.indexOf(severity);
    return at < 0 ? SEVERITIES.length : at;
  };
  return [...groups.values()].sort((a, b) => rank(a.severity) - rank(b.severity) || a.rule.localeCompare(b.rule));
}

/**
 * How many of `findings` the branch introduced and how many were there before it.
 * @param {any[]} findings
 */
function split(findings) {
  const introduced = findings.filter((finding) => finding.change === 'introduced').length;
  return `${introduced} new on this branch · ${findings.length - introduced} there before`;
}

/**
 * The fold of one rule's findings.
 * @param {ReturnType<typeof byRule>[number]} group
 * @param {any} report
 * @param {Map<string, string>} reasons
 * @param {string | undefined} key
 */
function ruleGroup(group, report, reasons, key) {
  const same = [...group.introduced, ...group.before].every((finding) => finding.message === group.message);
  const list = (/** @type {string} */ heading, /** @type {any[]} */ items) =>
    items.length
      ? [
          h('p', { class: 'finding-heading' }, heading),
          h('ul', { class: 'finding-lines' }, items.map((finding) => findingLine(finding, report, reasons, !same))),
        ]
      : null;
  // Beneath findings the branch introduced, those there before it are folded beneath their count.
  const older =
    group.introduced.length && group.before.length
      ? kept.details(
          { path: key ? `${key}/rule/${group.rule}/before` : null, open: false },
          { class: 'fold finding-before' },
          h('summary', { class: 'finding-heading' }, `There before (${group.before.length})`),
          h('ul', { class: 'finding-lines' }, group.before.map((finding) => findingLine(finding, report, reasons, !same))),
        )
      : list('There before', group.before);
  return kept.details(
    { path: key ? `${key}/rule/${group.rule}` : null, open: group.introduced.length > 0 },
    { class: 'fold finding-group', dataset: { rule: group.rule } },
    h(
      'summary',
      {},
      h('span', { class: 'rule' }, group.rule),
      ' ',
      chip(group.severity, group.severity),
      ' ',
      h('span', { class: 'finding-message' }, same ? group.message : `${group.introduced.length + group.before.length} findings`),
      ' ',
      h('span', { class: 'finding-counts' }, split([...group.introduced, ...group.before])),
    ),
    list('New on this branch', group.introduced),
    older,
  );
}

/**
 * One finding on one line: the node it names and where it is, and the agent's reason where it
 * left the finding in place.
 * @param {any} finding
 * @param {any} report
 * @param {Map<string, string>} reasons
 * @param {boolean} message whether to give the finding's own message, where its rule's differ
 */
function findingLine(finding, report, reasons, message) {
  const subject = finding.subjects?.[0];
  const reason = subject ? reasons.get(`${finding.rule} ${subject.key}`) : undefined;
  const node = subject ? report.nodes?.[subject.key] : undefined;
  const words = [message ? finding.message : null, node ? [node.label, node.name].filter(Boolean).join(' ') : null].filter(Boolean).join(' — ');
  return h(
    'li',
    { class: ['finding-line', finding.change === 'introduced' ? 'is-introduced' : null] },
    h(
      'span',
      { class: 'finding-what', title: words || null },
      message ? h('span', { class: 'finding-own' }, finding.message, ' ') : null,
      node ? h('span', { class: 'node' }, named(node)) : null,
    ),
    finding.path ? h('code', { class: 'site' }, place(finding)) : null,
    reason ? h('span', { class: 'reason' }, `Left in place: ${reason}`) : null,
  );
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
