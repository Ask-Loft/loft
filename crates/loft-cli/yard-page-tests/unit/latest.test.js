// Reads of the page that overlap: only the one begun last is applied.

import './dom.js';

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { latest } from '../../yard-page/lib/latest.js';

/** A read the test answers when it chooses. */
function held() {
  let answer;
  const promise = new Promise((resolve) => (answer = resolve));
  return { promise, answer };
}

test('of two reads under way at once, the one begun last is applied, whichever answers last', async () => {
  const read = latest();
  const first = held();
  const second = held();
  const earlier = read(first.promise);
  const later = read(second.promise);
  second.answer('branch B');
  first.answer('branch A');
  assert.equal(await later, 'branch B');
  assert.equal(await earlier, null, 'a read overtaken by a later one is not applied');
});

test('a read that no later one overtook is applied', async () => {
  const read = latest();
  assert.equal(await read(Promise.resolve('branch A')), 'branch A');
  assert.equal(await read(Promise.resolve('branch B')), 'branch B');
});
