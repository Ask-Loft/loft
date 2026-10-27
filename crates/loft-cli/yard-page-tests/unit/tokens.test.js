// The tokens Yard Log took.

import { forget } from './dom.js';

import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';

import { summarize } from '../../yard-page/model/tokens.js';
import { tokensView } from '../../yard-page/views/tokens.js';
import { act, log, stage } from './fixtures.js';

beforeEach(forget);

const counts = (input, cacheRead, cacheWrite, output) => ({ input, cacheRead, cacheWrite, output });

/** Two sessions' totals files over the stage `shaping`. */
function shown() {
  const shaping = stage('shape', 'Shaping', 'active', []);
  const files = [
    {
      session: 's1',
      firstEnded: '2026-01-01T10:00:00Z',
      lastEnded: '2026-01-01T10:30:00Z',
      tokens: counts(100, 1000, 10, 50),
      noStage: counts(20, 0, 0, 5),
      stages: [
        {
          stage: shaping.key,
          ...counts(80, 1000, 10, 45),
          agents: [
            { agent: 'main', ...counts(50, 900, 10, 35) },
            { agent: 'a1', type: 'Explore', ...counts(30, 100, 0, 10) },
          ],
        },
      ],
      agents: [
        { agent: 'main', model: 'opus', ...counts(70, 900, 10, 40) },
        { agent: 'a1', type: 'Explore', model: 'haiku', ...counts(30, 100, 0, 10) },
      ],
      submissions: [{ submission: 'sub', ...counts(10, 0, 0, 1) }],
    },
    {
      session: 's2',
      firstEnded: '2026-01-01T09:00:00Z',
      lastEnded: '2026-01-01T09:10:00Z',
      tokens: counts(1, 2, 3, 4),
      stages: [{ stage: shaping.key, ...counts(1, 2, 3, 4) }],
      agents: [{ agent: 'main', model: 'opus', ...counts(1, 2, 3, 4) }],
    },
  ];
  const sent = [{ key: 'sub', blocks: [], state: 'answered', changes: [{ what: 'sent', act: act('2026-01-01T10:10:00Z', 2) }] }];
  return log([shaping], { tokens: files, sent });
}

/** The rows of one breakdown, under its group row. @param {string} by */
function body(by) {
  return [...tokensView(shown()).querySelectorAll(`tbody[data-by="${by}"] tr:not(.group)`)];
}

test('the four counts in all are summed over every totals file', () => {
  assert.deepEqual(summarize(shown().tokens).all, counts(101, 1002, 13, 54));
  const totals = [...tokensView(shown()).querySelectorAll('.token-totals dd')].map((cell) => cell.getAttribute('title'));
  assert.deepEqual(totals, ['101', '1002', '13', '54']);
});

test('each stage has the counts of its requests, and those under no stage are shown apart', () => {
  const rows = body('stage').map((row) => [...row.cells].map((cell) => cell.textContent));
  assert.deepEqual(rows, [
    ['Shaping', '81', '1.0k', '13', '49'],
    ['Explore subagent', '30', '100', '0', '10'],
    ['Under no stage', '20', '0', '0', '5'],
  ]);
});

test('each session is listed with when its first and last counted requests ended and its counts, the earliest first', () => {
  const rows = body('session');
  assert.deepEqual(rows.map((row) => row.cells[0].querySelector('code').textContent), ['s2', 's1']);
  assert.match(rows[1].cells[0].querySelector('.span').textContent, /.+ – .+/);
  assert.equal(rows[1].cells[1].textContent, '100');
});

test('the main agent and each type of subagent have their counts for each model they ran on', () => {
  const rows = body('agent').map((row) => [row.cells[0].firstChild.textContent, row.cells[0].querySelector('.model').textContent, row.cells[1].textContent]);
  assert.deepEqual(rows, [
    ['Main agent', 'opus', '71'],
    ['Explore', 'haiku', '30'],
  ]);
});

test('each submission has the counts of the requests that ended while it was open', () => {
  const rows = body('submission').map((row) => [row.cells[0].textContent.startsWith('Sent'), row.cells[1].textContent]);
  assert.deepEqual(rows, [[true, '10']]);
});

test('the page explains that host-internal requests may be outside reported usage', () => {
  assert.match(tokensView(shown()).querySelector('.tokens-note').textContent, /Requests a host makes inside its own tools may be outside these counts/);
});

test('where no request has been counted, the page says that no tokens have been counted yet', () => {
  const view = tokensView(log([], { tokens: [] }));
  assert.equal(view.querySelector('.empty').textContent, 'No tokens have been counted yet.');
  assert.ok(!view.querySelector('table'));
});

test('the breakdowns share one table, whose columns are the four counts', () => {
  const view = tokensView(shown());
  assert.equal(view.querySelectorAll('table').length, 1);
  assert.deepEqual([...view.querySelectorAll('thead th')].map((cell) => cell.textContent), ['Input', 'Cache read', 'Cache write', 'Output']);
  assert.deepEqual(
    [...view.querySelectorAll('tbody')].map((group) => [group.dataset.by, group.querySelector('.group').textContent]),
    [
      ['stage', 'By stage'],
      ['session', 'By session'],
      ['agent', 'By agent and model'],
      ['submission', 'While a submission was open'],
    ],
  );
});

test('each measured breakdown count shows its share of the recorded log subtotal', () => {
  const [shaping, , noStage] = body('stage');
  const share = (cell) => parseFloat(cell.querySelector('.share-fill').style.width);
  assert.equal(share(shaping.cells[1]).toFixed(1), '80.2');
  assert.equal(share(noStage.cells[1]).toFixed(1), '19.8');
  assert.equal(share(shaping.cells[2]), 100);
  assert.equal(share(noStage.cells[2]), 0);
  assert.equal(shaping.cells[1].getAttribute('title'), '81 · 80% of recorded input');
  assert.equal(shaping.cells[1].querySelector('.share').getAttribute('aria-hidden'), 'true');
});

test('beside a stage that subagents counted toward, each type of subagent’s counts are shown within the stage’s', () => {
  const view = shown();
  const summary = summarize(view.tokens);
  const [shaping] = view.stages;
  assert.deepEqual([...(summary.stageAgents.get(shaping.key) ?? new Map())], [['Explore', counts(30, 100, 0, 10)]]);
  const rows = body('stage');
  const within = rows.filter((row) => row.classList.contains('within'));
  assert.deepEqual(within.map((row) => [...row.cells].map((cell) => cell.textContent)), [['Explore subagent', '30', '100', '0', '10']]);
  assert.equal(rows.indexOf(within[0]), 1, 'the subagent’s row follows its stage’s');
});

function legacyCodex(shaping) {
  return {
    host: 'codex', session: 'legacy', tokens: counts(10, 100, 0, 2),
    noStage: counts(0, 0, 0, 0),
    stages: [{ stage: shaping.key, ...counts(10, 100, 0, 2), agents: [{ agent: 'child', type: 'review', ...counts(10, 100, 0, 2) }] }],
    agents: [{ agent: 'child', type: 'review', model: 'fixture', ...counts(10, 100, 0, 2) }],
    submissions: [{ submission: 'sub', ...counts(10, 100, 0, 2) }],
  };
}

test('historical Codex zero writes are unavailable in every breakdown without mutating source data', () => {
  const shaping = stage('shape', 'Shaping', 'active', []);
  const file = legacyCodex(shaping);
  const original = structuredClone(file);
  const summary = summarize([file]);
  const values = [summary.all, summary.noStage, summary.stages.get(shaping.key), summary.stageAgents.get(shaping.key).get('review'), summary.agents[0].tokens, summary.sessions[0].tokens, summary.submissions.get('sub')];
  for (const counts of values) assert.equal(counts.cacheWrite, undefined);
  assert.equal(summary.all.input, 10);
  assert.equal(summary.all.cacheRead, 100);
  assert.equal(summary.all.output, 2);
  assert.equal(summary.partial, true);
  const view = tokensView(log([shaping], { tokens: [file] }));
  assert.equal(view.querySelectorAll('.token-totals dd')[2].textContent, 'Unavailable');
  for (const row of view.querySelectorAll('tbody tr:not(.group)')) {
    assert.equal(row.cells[3].textContent, 'Unavailable');
    assert.equal(row.cells[3].querySelector('.share'), null);
  }
  assert.deepEqual(file, original);
});

test('positive legacy writes and classified or Claude measured zeros retain their meaning', () => {
  const shaping = stage('shape', 'Shaping', 'active', []);
  for (const host of ['codex', 'claude-code']) {
    for (const classified of [undefined, false, true]) {
      for (const write of [0, 7]) {
        const file = { ...legacyCodex(shaping), host, tokens: counts(10, 100, write, 2), ...(classified === undefined ? {} : { cacheWriteClassified: classified }) };
        assert.equal(summarize([file]).all.cacheWrite, host === 'codex' && classified !== true && write === 0 ? undefined : write);
      }
    }
  }
});

test('mixed historical Codex and Claude totals preserve measured subtotals and partial coverage', () => {
  const shaping = stage('shape', 'Shaping', 'active', []);
  const codex = legacyCodex(shaping);
  const claude = { ...legacyCodex(shaping), host: 'claude-code', session: 'claude', tokens: counts(20, 200, 7, 3) };
  const summary = summarize([codex, claude]);
  assert.equal(summary.all.cacheWrite, 7);
  assert.equal(summary.all.input, 30);
  assert.ok(summary.all.partial.includes('cacheWrite'));
  assert.equal(summary.partial, true);
  assert.equal(summary.sessions.find((s) => s.host === 'codex').tokens.cacheWrite, undefined);
});

test('an empty stage uses classified session coverage rather than historical raw zero fields', () => {
  const shaping = stage('shape', 'Unstarted', 'planned', [], { owner: { host: 'codex', session: 'legacy' } });
  for (const classified of [false, true]) {
    const file = { ...legacyCodex(shaping), stages: [], cacheWriteClassified: classified };
    const view = tokensView(log([shaping], { tokens: [file] }));
    const row = view.querySelector('tbody[data-by="stage"] tr:not(.group)');
    assert.equal(row.cells[1].textContent, '0');
    assert.equal(row.cells[3].textContent, classified ? '0' : 'Unavailable');
  }
});
