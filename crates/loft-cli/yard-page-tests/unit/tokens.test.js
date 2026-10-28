// The tokens Yard Log took.

import { forget } from './dom.js';

import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';

import { tokensView } from '../../yard-page/views/tokens.js';
import { act, log, stage } from './fixtures.js';

beforeEach(forget);

const counts = (input, cacheRead, cacheWrite, output) => ({ input, cacheRead, cacheWrite, output });

/** The sums the page server gives of two sessions' totals files over the stage `shaping`. */
function shown() {
  const shaping = stage('shape', 'Shaping', 'active', []);
  const tokenSums = {
    all: counts(101, 1002, 13, 54),
    noStage: counts(20, 0, 0, 5),
    stages: [{ stage: shaping.key, tokens: counts(81, 1002, 13, 49), agents: [{ type: 'Explore', tokens: counts(30, 100, 0, 10) }] }],
    submissions: [{ submission: 'sub', tokens: counts(10, 0, 0, 1) }],
    agents: [
      { who: 'main agent', model: 'opus', tokens: counts(71, 902, 13, 44) },
      { who: 'Explore', model: 'haiku', tokens: counts(30, 100, 0, 10) },
    ],
    sessions: [
      { host: 'claude-code', session: 's2', firstEnded: '2026-01-01T09:00:00Z', lastEnded: '2026-01-01T09:10:00Z', tokens: counts(1, 2, 3, 4) },
      { host: 'claude-code', session: 's1', firstEnded: '2026-01-01T10:00:00Z', lastEnded: '2026-01-01T10:30:00Z', tokens: counts(100, 1000, 10, 50) },
    ],
    partial: false,
    counted: true,
  };
  const sent = [{ key: 'sub', blocks: [], state: 'answered', changes: [{ what: 'sent', act: act('2026-01-01T10:10:00Z', 2) }] }];
  return log([shaping], { tokenSums, sent });
}

/** The rows of one breakdown, under its group row. @param {string} by */
function body(by) {
  return [...tokensView(shown()).querySelectorAll(`tbody[data-by="${by}"] tr:not(.group)`)];
}

test('the four counts in all are summed over every totals file', () => {
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
  const view = tokensView(log([]));
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
  const rows = body('stage');
  const within = rows.filter((row) => row.classList.contains('within'));
  assert.deepEqual(within.map((row) => [...row.cells].map((cell) => cell.textContent)), [['Explore subagent', '30', '100', '0', '10']]);
  assert.equal(rows.indexOf(within[0]), 1, 'the subagent’s row follows its stage’s');
});

/** The sums the page server gives of a Codex totals file whose zero cache writes it holds unavailable. */
function legacyCodex(shaping) {
  const tokens = { input: 10, cacheRead: 100, output: 2 };
  return {
    all: { ...tokens, partial: ['cacheWrite'] },
    noStage: { input: 0, cacheRead: 0, output: 0 },
    stages: [{ stage: shaping.key, tokens, agents: [{ type: 'review', tokens }] }],
    submissions: [{ submission: 'sub', tokens }],
    agents: [{ who: 'codex review', model: 'fixture', tokens }],
    sessions: [{ host: 'codex', session: 'legacy', tokens }],
    partial: true,
    counted: true,
  };
}

test('cache writes the page server holds unavailable are shown unavailable in every breakdown', () => {
  const shaping = stage('shape', 'Shaping', 'active', []);
  const sent = [{ key: 'sub', blocks: [], state: 'answered', changes: [{ what: 'sent', act: act('2026-01-01T10:10:00Z', 2) }] }];
  const view = tokensView(log([shaping], { tokenSums: legacyCodex(shaping), sent }));
  assert.equal(view.querySelectorAll('.token-totals dd')[2].textContent, 'Unavailable');
  const rows = [...view.querySelectorAll('tbody tr:not(.group)')];
  assert.equal(rows.length, 5);
  for (const row of rows) {
    assert.equal(row.cells[3].textContent, 'Unavailable');
    assert.equal(row.cells[3].querySelector('.share'), null);
  }
});

test('an empty stage uses classified session coverage rather than historical raw zero fields', () => {
  const shaping = stage('shape', 'Unstarted', 'planned', [], { owner: { host: 'codex', session: 'legacy' } });
  for (const classified of [false, true]) {
    const tokenSums = { ...legacyCodex(shaping), stages: [] };
    if (classified) tokenSums.sessions = [{ host: 'codex', session: 'legacy', tokens: counts(10, 100, 0, 2) }];
    const view = tokensView(log([shaping], { tokenSums }));
    const row = view.querySelector('tbody[data-by="stage"] tr:not(.group)');
    assert.equal(row.cells[1].textContent, '0');
    assert.equal(row.cells[3].textContent, classified ? '0' : 'Unavailable');
  }
});
