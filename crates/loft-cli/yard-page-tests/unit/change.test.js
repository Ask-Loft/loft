// What a change the page server streams asks the page to load again.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { wantedBy } from '../../yard-page/model/change.js';

const shown = { tree: '/work/shelf', branch: 'feat/a' };

test('a change the page server no longer details loads the listing and the Yard Log shown', () => {
  assert.deepEqual(wantedBy({ all: true }, shown), { listing: true, log: true });
  assert.deepEqual(wantedBy({ trees: true, logs: [] }, shown), { listing: true, log: true });
});

test('only the sessions of the Yard Log shown changing loads it and not the listing', () => {
  const change = { logs: [{ tree: '/work/shelf', branch: 'feat/a', log: false }] };
  assert.deepEqual(wantedBy(change, shown), { listing: false, log: true });
});

test('another Yard Log changing loads the listing and not the Yard Log shown', () => {
  const change = { logs: [{ tree: '/work/shelf', branch: 'feat/b', log: true }] };
  assert.deepEqual(wantedBy(change, shown), { listing: true, log: false });
  const sessions = { logs: [{ tree: '/work/shelf', branch: 'feat/b', log: false }] };
  assert.deepEqual(wantedBy(sessions, shown), { listing: false, log: false });
});

test('with no Yard Log shown a change of one loads only the listing', () => {
  const change = { logs: [{ tree: '/work/shelf', branch: 'feat/a', log: true }] };
  assert.deepEqual(wantedBy(change, null), { listing: true, log: false });
});
