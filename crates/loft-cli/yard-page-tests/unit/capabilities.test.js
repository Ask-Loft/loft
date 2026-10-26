import { forget } from './dom.js';
import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';
import { tokensView } from '../../yard-page/views/tokens.js';
import { summarize } from '../../yard-page/model/tokens.js';
import { welcome } from '../../yard-page/views/trees.js';
import { log } from './fixtures.js';

beforeEach(forget);

const adapter = (usage) => ({ format: 'yard-adapter/1', version: '1', capabilities: { events: ['request'], usage, nativeQuestions: false, nativePlans: false, taskMirroring: false } });
const file = (session, tokens) => ({ host: 'other-agent', session, tokens });

test('missing counters display unavailable while measured zero displays zero', () => {
  const shown = log([], { tokens: [file('one', { input: 0, output: 5 })] });
  const cells = [...tokensView(shown).querySelectorAll('.token-totals dd')];
  assert.deepEqual(cells.map((cell) => cell.textContent), ['0', 'Unavailable', 'Unavailable', '5']);
});

test('a measured all-zero request is shown as measured usage', () => {
  const shown = log([], { tokens: [file('zero', { input: 0, cacheRead: 0, cacheWrite: 0, output: 0 })] });
  const view = tokensView(shown);
  assert.equal(view.querySelectorAll('.token-totals dd').length, 4);
  assert.equal(view.querySelector('.empty'), null);
});

test('mixed counter coverage displays recorded subtotals as partial', () => {
  const files = [file('one', { input: 2, output: 3 }), file('two', { output: 4 })];
  const summary = summarize(files);
  assert.equal(summary.all.input, 2);
  assert.ok(summary.all.partial.includes('input'));
  const view = tokensView(log([], { tokens: files }));
  assert.match(view.querySelector('.token-totals dd').textContent, /2.*partial/i);
  assert.equal(view.querySelector('tbody[data-by="session"] tr:not(.group)').cells[2].querySelector('.share'), null);
});

test('adapter coverage is shown with no token file and distinguishes unavailable from unknown', () => {
  const view = tokensView(log([], { tokens: [], activity: [
    { host: 'other-agent', session: 'one', adapter: adapter([]) },
    { host: 'legacy-agent', session: 'two' },
  ] }));
  assert.match(view.textContent, /other-agent/);
  assert.match(view.textContent, /unavailable/i);
  assert.match(view.textContent, /legacy-agent/);
  assert.match(view.textContent, /unknown/i);
});

test('an unreporting session makes the observed aggregate partial', () => {
  const view = tokensView(log([], { tokens: [file('one', { input: 2, output: 3 })], activity: [
    { host: 'other-agent', session: 'two', adapter: adapter([]) },
  ] }));
  assert.match(view.querySelector('.token-totals dd').textContent, /partial/i);
});

test('the empty page describes an agent host without requiring Claude', () => {
  const view = welcome({ repositories: [] });
  assert.match(view.textContent, /agent/);
  assert.doesNotMatch(view.textContent, /Claude Code/);
});
