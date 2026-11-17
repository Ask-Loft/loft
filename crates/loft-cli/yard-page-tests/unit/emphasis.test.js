// The hover layer of a diagram the page draws: pointing at a node brings it, its edges and its
// neighbors forward and fades the rest; leaving it lets them fall back.

import { forget, root } from './dom.js';

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { beforeEach, test } from 'node:test';

import { emphasis } from '../../yard-page/lib/emphasis.js';

beforeEach(forget);

/** A drawn chain A → B → C and a lone D, in a target with its hover layer. */
function chain() {
  const target = document.createElement('div');
  target.innerHTML = `<svg class="flow">
    <path class="flow-edge" data-from="A" data-to="B"></path><g class="flow-pill" data-from="A" data-to="B"></g>
    <path class="flow-edge" data-from="B" data-to="C"></path>
    <g class="flow-node" data-id="A"><rect></rect></g><g class="flow-node" data-id="B"><rect></rect></g>
    <g class="flow-node" data-id="C"><rect></rect></g><g class="flow-node" data-id="D"><rect></rect></g>
  </svg>`;
  root().replaceChildren(target);
  emphasis(target);
  return target;
}

const near = (target) => [...target.querySelectorAll('.is-near')].map((each) => each.getAttribute('data-id') ?? `${each.getAttribute('data-from')}>${each.getAttribute('data-to')}`);

test('pointing at a node marks it, its edges and their labels and the nodes they join, and nothing else', () => {
  const target = chain();
  target.querySelector('[data-id="A"] rect').dispatchEvent(new Event('pointerover', { bubbles: true }));
  assert.ok(target.querySelector('svg').classList.contains('is-emphasis'));
  assert.deepEqual(near(target).sort(), ['A', 'A>B', 'A>B', 'B'].sort());
  target.querySelector('[data-id="B"] rect').dispatchEvent(new Event('pointerover', { bubbles: true }));
  assert.deepEqual(near(target).sort(), ['A', 'A>B', 'A>B', 'B', 'B>C', 'C'].sort(), 'a node between two brings both forward');
});

test('the marks stay until the pointer leaves the node, moving within it keeps them, and nothing fades', () => {
  const target = chain();
  const node = target.querySelector('[data-id="A"]');
  node.querySelector('rect').dispatchEvent(new Event('pointerover', { bubbles: true }));
  const within = new Event('pointerout', { bubbles: true });
  Object.defineProperty(within, 'relatedTarget', { value: node });
  node.querySelector('rect').dispatchEvent(within);
  assert.ok(target.querySelector('svg').classList.contains('is-emphasis'), 'still within the node');
  node.querySelector('rect').dispatchEvent(new Event('pointerout', { bubbles: true }));
  assert.ok(!target.querySelector('svg').classList.contains('is-emphasis'));
  assert.deepEqual(near(target), []);
  const css = readFileSync(new URL('../../yard-page/yard.css', import.meta.url), 'utf8');
  assert.doesNotMatch(css, /is-emphasis[^{]*\{[^}]*opacity/, 'nothing fades');
  assert.match(css, /\.flow\.is-emphasis \.flow-edge\.is-near \{ stroke: var\(--signal\);/, 'its edges in action blue');
  assert.match(css, /\.flow\.is-emphasis \.flow-node\.is-near :is\(rect, polygon, circle\) \{ stroke: var\(--signal\);/, 'its nodes outlined in action blue');
  assert.match(css, /\.flow\.is-emphasis \.spec-card\.is-near \.spec-frame \{ stroke: var\(--signal\);/);
});
