// The tokens Yard Log took: the four counts in all, by stage, by session, by agent and model,
// and by submission.

import { add, h } from '../lib/dom.js';
import { tokens as amount, when } from '../lib/format.js';
import { summarize, total, zero } from '../model/tokens.js';

const COUNTS = [
  ['input', 'Input'],
  ['cacheRead', 'Cache read'],
  ['cacheWrite', 'Cache write'],
  ['output', 'Output'],
];

/** @param {import('../model/tokens.js').Counts} counts */
function cells(counts) {
  return COUNTS.map(([name]) => h('td', { class: 'number', title: String(counts[name]) }, amount(counts[name])));
}

/** @param {string} first */
function head(first) {
  return h('thead', {}, h('tr', {}, h('th', { scope: 'col' }, first), COUNTS.map(([, word]) => h('th', { scope: 'col', class: 'number' }, word))));
}

/**
 * @param {string} first
 * @param {[any, import('../model/tokens.js').Counts][]} rows
 */
function table(first, rows) {
  return h(
    'div',
    { class: 'table-wrap' },
    h('table', { class: 'tokens-table' }, head(first), h('tbody', {}, rows.map(([name, counts]) => h('tr', {}, h('th', { scope: 'row' }, name), cells(counts))))),
  );
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
  const stages = (log.stages ?? []).map((stage) => [stage.version.body.title, summary.stages.get(stage.key) ?? zero()]);
  if (total(summary.noStage)) stages.push(['Under no stage', summary.noStage]);
  const submissions = (log.sent ?? [])
    .filter((submission) => summary.submissions.has(submission.key))
    .map((submission) => {
      const sent = submission.changes.find((change) => change.what === 'sent');
      return [`Sent ${when(sent?.act.at)}`, summary.submissions.get(submission.key)];
    });
  add(
    section,
    h('dl', { class: 'token-totals' }, COUNTS.map(([name, word]) => [h('dt', {}, word), h('dd', { title: String(summary.all[name]) }, amount(summary.all[name]))])),
    h('h4', {}, 'By stage'),
    table('Stage', stages),
    h('h4', {}, 'By session'),
    table(
      'Session',
      summary.sessions.map((session) => [
        [h('code', {}, session.session), h('span', { class: 'span' }, ` ${when(session.firstEnded)} to ${when(session.lastEnded)}`)],
        session.tokens,
      ]),
    ),
    h('h4', {}, 'By agent and model'),
    table('Agent', summary.agents.map((agent) => [`${agent.who} on ${agent.model}`, agent.tokens])),
    submissions.length ? [h('h4', {}, 'While a submission was open'), table('Submission', submissions)] : null,
    note,
  );
  return section;
}
