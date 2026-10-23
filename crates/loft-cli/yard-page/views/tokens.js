// The tokens Yard Log took: the four counts in all, by stage with each type of subagent's share
// beneath it, by session, by agent and model, and by submission.

import { add, h } from '../lib/dom.js';
import { tokens as amount, when } from '../lib/format.js';
import { summarize, total, zero } from '../model/tokens.js';

const COUNTS = [
  ['input', 'Input'],
  ['cacheRead', 'Cache read'],
  ['cacheWrite', 'Cache write'],
  ['output', 'Output'],
];

/**
 * A count, with a bar of its share of Yard Log's count of that kind in all.
 * @param {number} count
 * @param {number} all
 * @param {string} word
 */
function cell(count, all, word) {
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
            h('tr', kind ? { class: kind } : {}, h('th', { scope: 'row' }, name), COUNTS.map(([key, word]) => cell(counts[key], all[key], word))),
          ),
        ),
      ),
    ),
  );
}

/**
 * A session, by the start of its id, with when its first and last counted requests ended.
 * @param {{ session: string, firstEnded: string, lastEnded: string }} session
 */
function sessionName(session) {
  return [
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
  const summary = summarize(log.tokens);
  const section = h('section', { class: 'tokens', id: 'tokens', 'aria-labelledby': 'tokens-title' }, h('h3', { id: 'tokens-title' }, 'Tokens'));
  const note = h(
    'p',
    { class: 'tokens-note' },
    'Requests that Claude Code makes inside its own tools, such as a web search or a web fetch, are not counted.',
  );
  if (!summary.counted) {
    add(section, h('p', { class: 'empty' }, 'No tokens have been counted yet.'), note);
    return section;
  }
  const stages = (log.stages ?? []).flatMap((stage) => [
    [stage.version.body.title, summary.stages.get(stage.key) ?? zero()],
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
    h('dl', { class: 'token-totals' }, COUNTS.map(([name, word]) => h('div', {}, h('dt', {}, word), h('dd', { title: String(summary.all[name]) }, amount(summary.all[name]))))),
    breakdowns(groups, summary.all),
    note,
  );
  return section;
}
