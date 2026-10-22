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

test('the four counts in all are summed over every totals file', () => {
  assert.deepEqual(summarize(shown().tokens).all, counts(101, 1002, 13, 54));
  const totals = [...tokensView(shown()).querySelectorAll('.token-totals dd')].map((cell) => cell.getAttribute('title'));
  assert.deepEqual(totals, ['101', '1002', '13', '54']);
});

test('each stage has the counts of its requests, and those under no stage are shown apart', () => {
  const rows = [...tokensView(shown()).querySelectorAll('table')[0].querySelectorAll('tbody tr')].map((row) => [...row.cells].map((cell) => cell.textContent));
  assert.deepEqual(rows, [
    ['Shaping', '81', '1.0k', '13', '49'],
    ['Under no stage', '20', '0', '0', '5'],
  ]);
});

test('each session is listed with when its first and last counted requests ended and its counts, the earliest first', () => {
  const rows = [...tokensView(shown()).querySelectorAll('table')[1].querySelectorAll('tbody tr')];
  assert.deepEqual(rows.map((row) => row.cells[0].querySelector('code').textContent), ['s2', 's1']);
  assert.match(rows[1].cells[0].textContent, /s1 .* to /);
  assert.equal(rows[1].cells[1].textContent, '100');
});

test('the main agent and each type of subagent have their counts for each model they ran on', () => {
  const rows = [...tokensView(shown()).querySelectorAll('table')[2].querySelectorAll('tbody tr')].map((row) => [row.cells[0].textContent, row.cells[1].textContent]);
  assert.deepEqual(rows, [
    ['main agent on opus', '71'],
    ['Explore on haiku', '30'],
  ]);
});

test('each submission has the counts of the requests that ended while it was open', () => {
  const rows = [...tokensView(shown()).querySelectorAll('table')[3].querySelectorAll('tbody tr')].map((row) => [row.cells[0].textContent.startsWith('Sent'), row.cells[1].textContent]);
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
