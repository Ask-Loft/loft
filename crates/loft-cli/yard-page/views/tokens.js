// The tokens Yard Log took: the four counts in all, by stage with each type of subagent's share
// beneath it, by session, by agent and model, and by submission.

import { add, h } from '../lib/dom.js';
import { tokens as amount, when } from '../lib/format.js';
import { COUNTERS as KEYS, summarize, total } from '../model/tokens.js';

const COUNTS = [
  ['input', 'Input'],
  ['cacheRead', 'Cache read'],
  ['cacheWrite', 'Cache write'],
  ['output', 'Output'],
];

/**
 * A count, with a bar of its share of Yard Log's count of that kind in all.
 * @param {number | undefined} count
 * @param {number | undefined} all
 * @param {string} word
 * @param {boolean} [partial]
 */
function cell(count, all, word, partial = false) {
  if (count === undefined) return h('td', { class: 'number', title: 'Unavailable' }, 'Unavailable');
  if (partial) return h('td', { class: 'number', title: `${count} recorded; coverage is partial` }, `${amount(count)} (partial)`);
  const share = all ? count / all : 0;
  return h(
    'td',
    { class: 'number', title: `${count.toLocaleString()} · ${Math.round(share * 100)}% of all ${word.toLowerCase()}` },
    amount(count),
    h('span', { class: 'share', 'aria-hidden': 'true' }, h('span', { class: 'share-fill', style: `width:${share * 100}%` })),
  );
}

/**
 * Every breakdown in one table, whose columns are the four counts: a group of rows for each.
 * @param {{ by: string, title: string, rows: [any, import('../model/tokens.js').Counts, string?][] }[]} groups
 * @param {import('../model/tokens.js').Counts} all
 */
function breakdowns(groups, all) {
  return h(
    'div',
    { class: 'table-wrap' },
    h(
      'table',
      { class: 'tokens-table' },
      h('thead', {}, h('tr', {}, h('td', {}), COUNTS.map(([, word]) => h('th', { scope: 'col', class: 'number' }, word)))),
      groups.map(({ by, title, rows }) =>
        h(
          'tbody',
          { dataset: { by } },
          h('tr', { class: 'group' }, h('th', { scope: 'rowgroup', colspan: String(COUNTS.length + 1) }, title)),
          rows.map(([name, counts, kind]) =>
            h('tr', kind ? { class: kind } : {}, h('th', { scope: 'row' }, name), COUNTS.map(([key, word]) => cell(counts[key], all[key], word, counts.partial?.includes(key) || all.partial?.includes(key)))),
          ),
        ),
      ),
    ),
  );
}

/**
 * A session, by the start of its id, with when its first and last counted requests ended.
 * @param {{ host?: string, session: string, firstEnded: string, lastEnded: string }} session
 */
function sessionName(session) {
  return [
    session.host ? h('span', { class: 'host' }, `${session.host} · `) : null,
    h('code', { title: session.session }, String(session.session).slice(0, 8)),
    h('span', { class: 'span' }, `${when(session.firstEnded)} – ${when(session.lastEnded)}`),
  ];
}

/**
 * An agent, by who it is, and the model it ran on.
 * @param {{ who: string, model: string }} agent
 */
function agentName(agent) {
  return [agent.who.charAt(0).toUpperCase() + agent.who.slice(1), h('span', { class: 'model' }, agent.model)];
}

/**
 * The tokens section of a Yard Log.
 * @param {any} log
 */
export function tokensView(log) {
  const summary = summarize(log.tokens, log.activity);
  const section = h('section', { class: 'tokens', id: 'tokens', 'aria-labelledby': 'tokens-title' }, h('h3', { id: 'tokens-title' }, 'Tokens'));
  const note = h(
    'p',
    { class: 'tokens-note' },
    'Usage includes requests reported by each adapter. Requests a host makes inside its own tools may be outside these counts.',
  );
  const reporting = reportingOf(log.activity ?? []);
  if (!summary.counted) {
    add(section, h('p', { class: 'empty' }, 'No tokens have been counted yet.'), reporting, note);
    return section;
  }
  const stages = (log.stages ?? []).flatMap((stage) => [
    [stage.version.body.title, summary.stages.get(stage.key) ?? zeroFor(log, stage.owner)],
    // Each type of subagent that worked for the stage, its counts within the stage's.
    ...[...(summary.stageAgents.get(stage.key) ?? [])]
      .sort(([, a], [, b]) => total(b) - total(a))
      .map(([type, counts]) => [`${type} subagent`, counts, 'within']),
  ]);
  if (total(summary.noStage)) stages.push([h('span', { class: 'no-stage' }, 'Under no stage'), summary.noStage]);
  const submissions = (log.sent ?? [])
    .filter((submission) => summary.submissions.has(submission.key))
    .map((submission) => {
      const sent = submission.changes.find((change) => change.what === 'sent');
      return [`Sent ${when(sent?.act.at)}`, summary.submissions.get(submission.key)];
    });
  const groups = [
    { by: 'stage', title: 'By stage', rows: stages },
    { by: 'session', title: 'By session', rows: summary.sessions.map((session) => [sessionName(session), session.tokens]) },
    { by: 'agent', title: 'By agent and model', rows: summary.agents.map((agent) => [agentName(agent), agent.tokens]) },
    { by: 'submission', title: 'While a submission was open', rows: submissions },
  ].filter((group) => group.rows.length);
  add(
    section,
    h('dl', { class: 'token-totals' }, COUNTS.map(([name, word]) => h('div', {}, h('dt', {}, word), h('dd', { title: summary.all[name] === undefined ? 'Unavailable' : String(summary.all[name]) }, summary.all[name] === undefined ? 'Unavailable' : `${amount(summary.all[name])}${summary.all.partial?.includes(name) ? ' (partial)' : ''}`)))),
    breakdowns(groups, summary.all),
    reporting,
    note,
  );
  return section;
}

/** Recorded zero only for counters this session actually reports. @param {any} log @param {any} owner */
function zeroFor(log, owner) {
  const file = (log.tokens ?? []).find((file) => file.session === owner?.session && (file.host ?? 'claude-code') === (owner?.host ?? 'claude-code'));
  return Object.fromEntries(KEYS.filter((key) => file?.tokens?.[key] !== undefined).map((key) => [key, 0]));
}

/** The integration available from each session, including ones with no measured usage. @param {any[]} activities */
function reportingOf(activities) {
  if (!activities.length) return null;
  return h('div', { class: 'reporting-coverage' }, h('h4', {}, 'Reporting coverage'), h('ul', {}, activities.map((activity) => {
    const host = activity.host ?? 'claude-code';
    const descriptor = activity.adapter;
    if (!descriptor) return h('li', {}, `${host} · ${activity.session}: reporting coverage unknown.`);
    const caps = descriptor.capabilities ?? {};
    const available = (flag) => flag ? 'available' : 'unavailable';
    const usage = (caps.usage ?? []).length ? `Usage: ${caps.usage.join(', ')}.` : 'Usage unavailable.';
    return h('li', {}, `${host} · ${activity.session} (${descriptor.version}): ${usage} Activity ${available((caps.events ?? []).length)}. Native questions ${available(caps.nativeQuestions)}. Native plans ${available(caps.nativePlans)}. Task mirroring ${available(caps.taskMirroring)}.`);
  })));
}
