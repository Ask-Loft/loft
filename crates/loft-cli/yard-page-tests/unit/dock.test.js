// The height of the panel of what waits: half the viewport at first, then as the person drags its
// top edge or moves it with the arrow keys, within its bounds, and kept in this browser.

import { forget, root } from './dom.js';

import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';

import { ABOVE, LEAST, dockHeight } from '../../yard-page/lib/dock.js';
import * as modal from '../../yard-page/lib/modal.js';
import * as prefs from '../../yard-page/lib/prefs.js';

const window = /** @type {any} */ (globalThis.window);

/** The viewport `height` pixels high, as the browser tells the page once it is resized. */
function viewport(height) {
  window.happyDOM.setViewport({ width: 1280, height });
  window.dispatchEvent(new window.Event('resize'));
}

/** The panel's height as the page sets it, in pixels. */
function height() {
  return Number.parseFloat(document.documentElement.style.getPropertyValue('--dock-height'));
}

/** Opens the panel `shelf`, and gives its top edge. */
function panel() {
  modal.show('shelf', { title: 'The shelf', content: 'Oak', docked: true });
  return /** @type {any} */ (document.querySelector('dialog.modal[data-modal="shelf"] .dock-edge'));
}

/** @param {string} type @param {number} clientY */
function pointer(type, clientY) {
  return Object.assign(new window.Event(type), { clientY, pointerId: 1 });
}

/** @param {string} key */
function key(key) {
  return new window.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
}

beforeEach(() => {
  modal.close('shelf');
  forget();
  prefs.set('dock-height', null);
  root();
  document.documentElement.style.removeProperty('--dock-height');
  viewport(900);
});

test('the panel first opens at half the viewport’s height', () => {
  const edge = panel();
  assert.equal(height(), 450);
  assert.equal(edge.getAttribute('aria-valuenow'), '450');
});

test('dragging the panel’s top edge makes the panel taller or shorter by the length dragged', () => {
  const edge = panel();
  edge.dispatchEvent(pointer('pointerdown', 450));
  edge.dispatchEvent(pointer('pointermove', 350));
  assert.equal(height(), 550);
  edge.dispatchEvent(pointer('pointermove', 500));
  assert.equal(height(), 400);
  edge.dispatchEvent(pointer('pointerup', 500));
  assert.equal(height(), 400);
});

test('the edge is reached by the keyboard, and the arrow keys up and down make the panel taller and shorter', () => {
  const edge = panel();
  assert.equal(edge.getAttribute('role'), 'separator');
  assert.equal(edge.getAttribute('aria-orientation'), 'horizontal');
  assert.equal(edge.getAttribute('tabindex'), '0');
  const up = key('ArrowUp');
  edge.dispatchEvent(up);
  assert.ok(up.defaultPrevented);
  assert.ok(height() > 450);
  edge.dispatchEvent(key('ArrowDown'));
  edge.dispatchEvent(key('ArrowDown'));
  assert.ok(height() < 450);
  assert.ok(modal.isOpen('shelf'), 'the keys move the edge and close nothing');
});

test('the panel is never shorter than 160 pixels, and always leaves at least 120 pixels of the page above it, also as the viewport changes', () => {
  assert.deepEqual([LEAST, ABOVE], [160, 120]);
  assert.equal(dockHeight(40, 900, 24), 160);
  assert.equal(dockHeight(2000, 900, 24), 900 - 24 - 120);
  assert.equal(dockHeight(null, 900, 24), 450);
  const edge = panel();
  edge.dispatchEvent(pointer('pointerdown', 450));
  edge.dispatchEvent(pointer('pointermove', -1000));
  assert.equal(height(), 900 - 24 - 120);
  edge.dispatchEvent(pointer('pointermove', 2000));
  assert.equal(height(), 160);
  edge.dispatchEvent(pointer('pointerup', 2000));
  edge.dispatchEvent(pointer('pointerdown', 450));
  edge.dispatchEvent(pointer('pointermove', 0));
  edge.dispatchEvent(pointer('pointerup', 0));
  assert.equal(height(), 610);
  viewport(600);
  assert.equal(height(), 600 - 24 - 120);
});

test('the height chosen is the panel’s height the next time it opens in this browser, and a resize chooses none', () => {
  const edge = panel();
  edge.dispatchEvent(pointer('pointerdown', 450));
  edge.dispatchEvent(pointer('pointermove', 350));
  edge.dispatchEvent(pointer('pointerup', 350));
  viewport(600);
  modal.close('shelf');
  viewport(900);
  document.documentElement.style.removeProperty('--dock-height');
  panel();
  assert.equal(height(), 550);
});
