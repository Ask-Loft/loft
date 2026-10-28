// Token totals remain visible while detailed accounting is a per-log remembered fold.
import { forget } from './dom.js';
import assert from 'node:assert/strict';
import { afterEach, beforeEach, test } from 'node:test';
import * as kept from '../../yard-page/lib/kept.js';
import { tokensView } from '../../yard-page/views/tokens.js';
import { log } from './fixtures.js';

beforeEach(() => { forget(); kept.reset(); });
afterEach(() => { kept.flush(); kept.reset(); });

function shown(key = 'log-a', counted = true) {
  const doc = log([], { tokens: counted ? [{
    host: 'claude-code', session: 's1', firstEnded: '2026-01-01T10:00:00Z', lastEnded: '2026-01-01T10:01:00Z',
    tokens: { input: 10, cacheRead: 100, cacheWrite: 1, output: 2, partial: ['input'] },
  }] : [], activity: [{ host: 'claude-code', session: 's1' }] });
  doc.yardLog.key = key;
  return doc;
}
function draw(doc) { kept.begin(doc); return tokensView(doc); }
function toggle(fold) { fold.open = !fold.open; fold.dispatchEvent(new Event('toggle')); }

test('token details start closed with the totals and partial notice outside the fold', () => {
  const view = draw(shown());
  const fold = view.querySelector('details.token-details');
  assert.ok(fold, 'details have a native fold');
  assert.equal(fold.open, false);
  const totals = view.querySelector('.token-totals');
  assert.equal(totals.querySelectorAll('dd').length, 4);
  assert.deepEqual([...totals.querySelectorAll('dd')].map(dd => dd.title), ['10', '100', '1', '2']);
  assert.equal(fold.contains(totals), false);
  const coverage = view.querySelector('.token-coverage');
  assert.match(coverage.textContent, /partial/);
  assert.equal(fold.contains(coverage), false);
  for (const selector of ['.tokens-table', '.reporting-coverage', '.tokens-note']) {
    assert.ok(fold.querySelector(selector), `${selector} is inside the closed fold`);
  }
  toggle(fold);
  assert.equal(fold.open, true);
  toggle(fold);
  assert.equal(fold.open, false);
});

test('the no tokens summary stays outside closed token details', () => {
  const view = draw(shown('empty', false));
  const fold = view.querySelector('details.token-details');
  assert.ok(fold);
  assert.equal(fold.open, false);
  const empty = view.querySelector('.empty');
  assert.equal(empty.textContent, 'No tokens have been counted yet.');
  assert.equal(fold.contains(empty), false);
  assert.ok(fold.querySelector('.reporting-coverage'));
  assert.ok(fold.querySelector('.tokens-note'));
  assert.ok(!view.querySelector('table'));
});

test('the token fold choice survives usage redraws and reloads independently for each log', () => {
  const doc = shown();
  toggle(draw(doc).querySelector('details.token-details'));
  doc.tokens[0].tokens.input = 25;
  let view = draw(doc);
  assert.equal(view.querySelector('details.token-details').open, true);
  assert.equal(view.querySelector('.token-totals dd').title, '25');
  kept.flush();
  kept.reset();
  assert.equal(draw(doc).querySelector('details.token-details').open, true);
  assert.equal(draw(shown('log-b')).querySelector('details.token-details').open, false);
  view = draw(doc);
  assert.equal(view.querySelector('details.token-details').open, true);
  toggle(view.querySelector('details.token-details'));
  kept.flush();
  kept.reset();
  assert.equal(draw(doc).querySelector('details.token-details').open, false);
});

test('the token fold stays open when an empty summary receives its first counts', () => {
  const doc = shown('first', false);
  toggle(draw(doc).querySelector('details.token-details'));
  doc.tokens = shown().tokens;
  const view = draw(doc);
  assert.equal(view.querySelector('details.token-details').open, true);
  assert.ok(!view.querySelector('.empty'));
  assert.equal(view.querySelector('.token-totals dd').title, '10');
});
