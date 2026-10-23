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
      stages: [{ stage: shaping.key, ...counts(80, 1000, 10, 45) }],
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

test('the page says that requests made inside Claude Code’s own tools are not counted', () => {
  assert.match(tokensView(shown()).querySelector('.tokens-note').textContent, /web search or a web fetch, are not counted/);
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

test('each count of a breakdown shows as a bar its share of Yard Log’s count of that kind in all', () => {
  const [shaping, noStage] = body('stage');
  const share = (cell) => parseFloat(cell.querySelector('.share-fill').style.width);
  assert.equal(share(shaping.cells[1]).toFixed(1), '80.2');
  assert.equal(share(noStage.cells[1]).toFixed(1), '19.8');
  assert.equal(share(shaping.cells[2]), 100);
  assert.equal(share(noStage.cells[2]), 0);
  assert.equal(shaping.cells[1].getAttribute('title'), '81 · 80% of all input');
  assert.equal(shaping.cells[1].querySelector('.share').getAttribute('aria-hidden'), 'true');
});
