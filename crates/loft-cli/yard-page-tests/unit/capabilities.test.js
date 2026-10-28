import { forget } from './dom.js';
import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';
import { tokensView } from '../../yard-page/views/tokens.js';
import { welcome } from '../../yard-page/views/trees.js';
import { log, stage } from './fixtures.js';

beforeEach(forget);

const adapter = (usage) => ({ format: 'yard-adapter/1', version: '1', capabilities: { events: ['request'], usage, nativeQuestions: false, nativePlans: false, taskMirroring: false } });
/** The sums the page server gives of one other host's session holding `tokens`. */
const sums = (tokens, partial = false) => ({
  all: tokens, noStage: {}, stages: [], submissions: [], agents: [],
  sessions: [{ host: 'other-agent', session: 'one', tokens }],
  partial, counted: Object.keys(tokens).some((key) => key !== 'partial'),
});

test('missing counters display unavailable while measured zero displays zero', () => {
  const shown = log([], { tokenSums: sums({ input: 0, output: 5 }, true) });
  const cells = [...tokensView(shown).querySelectorAll('.token-totals dd')];
  assert.deepEqual(cells.map((cell) => cell.textContent), ['0', 'Unavailable', 'Unavailable', '5']);
});

test('a measured all-zero request is shown as measured usage', () => {
  const shown = log([], { tokenSums: sums({ input: 0, cacheRead: 0, cacheWrite: 0, output: 0 }) });
  const view = tokensView(shown);
  assert.equal(view.querySelectorAll('.token-totals dd').length, 4);
  assert.equal(view.querySelector('.empty'), null);
  assert.equal(view.querySelector('.token-coverage'), null);
});

test('token accounting completeness is shown as the page server derives it', () => {
  const complete = { input: 0, cacheRead: 0, cacheWrite: 0, output: 0 };
  assert.equal(tokensView(log([], { tokenSums: sums(complete) })).querySelector('.token-coverage'), null);
  assert.match(tokensView(log([], { tokenSums: sums(complete, true) })).querySelector('.token-coverage').textContent, /Token accounting: partial/);
  const unreporting = { ...sums({}, true), sessions: [] };
  assert.ok(tokensView(log([], { tokenSums: unreporting })).querySelector('.token-coverage'));
  assert.equal(tokensView(log([])).querySelector('.token-coverage'), null);
});

test('mixed counter coverage marks the log partial while keeping numeric subtotals', () => {
  const tokenSums = {
    ...sums({ input: 2, output: 7, partial: ['input'] }, true),
    sessions: [
      { host: 'other-agent', session: 'one', tokens: { input: 2, output: 3 } },
      { host: 'other-agent', session: 'two', tokens: { output: 4 } },
    ],
  };
  const view = tokensView(log([], { tokenSums }));
  assert.equal(view.querySelector('.token-totals dd').textContent, '2');
  assert.match(view.querySelector('.token-coverage').textContent, /Token accounting: partial/);
  assert.equal(view.querySelector('tbody[data-by="session"] tr:not(.group)').cells[1].querySelector('.share-fill').style.width, '100%');
  assert.equal(view.querySelector('tbody[data-by="session"] tr:not(.group)').cells[2].querySelector('.share'), null);
});

test('partial log coverage is shown once while breakdown bars use recorded subtotals', () => {
  const complete = { input: 10, cacheRead: 20, cacheWrite: 0, output: 5 };
  const incomplete = { ...complete, partial: ['input'] };
  const fullStage = stage('complete', 'Complete stage', 'done', []);
  const partialStage = stage('partial', 'Partial stage', 'done', []);
  const tokenSums = {
    ...sums({ input: 20, cacheRead: 40, cacheWrite: 0, output: 10, partial: ['input'] }, true),
    noStage: { input: 0, cacheRead: 0, cacheWrite: 0, output: 0, partial: ['input'] },
    stages: [
      { stage: fullStage.key, tokens: complete, agents: [{ type: 'Explore', tokens: complete }] },
      { stage: partialStage.key, tokens: incomplete, agents: [] },
    ],
    agents: [{ who: 'other-agent main agent', model: 'fixture', tokens: complete }],
    sessions: [
      { host: 'other-agent', session: 'one', tokens: complete },
      { host: 'other-agent', session: 'two', tokens: incomplete },
    ],
  };
  const view = tokensView(log([fullStage, partialStage], { tokenSums }));
  assert.equal(view.querySelector('.token-totals dd').textContent, '20');
  assert.equal(view.querySelectorAll('.token-coverage').length, 1);
  assert.equal((view.textContent.match(/Token accounting: partial/g) ?? []).length, 1);
  const stageRows = [...view.querySelectorAll('tbody[data-by="stage"] tr:not(.group)')];
  assert.equal(stageRows[0].cells[1].textContent, '10');
  assert.equal(stageRows[1].cells[1].textContent, '10');
  assert.equal(stageRows[2].cells[1].textContent, '10');
  assert.equal(stageRows[0].cells[1].querySelector('.share-fill').style.width, '50%');
  assert.equal(stageRows[2].cells[1].querySelector('.share-fill').style.width, '50%');
  assert.match(stageRows[0].cells[1].title, /50% of recorded input/);
  assert.match(stageRows[2].cells[1].title, /50% of recorded input/);
  assert.ok(stageRows[0].cells[4].querySelector('.share'));
  assert.equal(view.querySelector('tbody[data-by="session"] tr:not(.group)').cells[1].textContent, '10');
  assert.equal(view.querySelector('tbody[data-by="agent"] tr:not(.group)').cells[1].textContent, '10');
});

test('adapter coverage is shown with no token file and distinguishes unavailable from unknown', () => {
  const view = tokensView(log([], { activity: [
    { host: 'other-agent', session: 'one', adapter: adapter([]) },
    { host: 'legacy-agent', session: 'two' },
  ] }));
  assert.match(view.textContent, /other-agent/);
  assert.match(view.textContent, /unavailable/i);
  assert.match(view.textContent, /legacy-agent/);
  assert.match(view.textContent, /unknown/i);
});

test('an unreporting session marks token accounting partial at log level', () => {
  const view = tokensView(log([], { tokenSums: sums({ input: 2, cacheRead: 0, cacheWrite: 0, output: 3, partial: ['input', 'cacheRead', 'cacheWrite', 'output'] }, true), activity: [
    { host: 'other-agent', session: 'two', adapter: adapter([]) },
  ] }));
  assert.equal(view.querySelector('.token-totals dd').textContent, '2');
  assert.match(view.querySelector('.token-coverage').textContent, /Token accounting: partial/);
});

test('the empty page describes an agent host without requiring Claude', () => {
  const view = welcome({ repositories: [] });
  assert.match(view.textContent, /agent/);
  assert.doesNotMatch(view.textContent, /Claude Code/);
});
