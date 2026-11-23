// The width of the open sidebar: 248 pixels at first, then as the person drags its right edge or
// moves it with the arrow keys, within its bounds, and kept in this browser.

import { forget } from './dom.js';

import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';

import * as prefs from '../../yard-page/lib/prefs.js';
import { BESIDE, FIRST, LEAST, MOST, edge, sideWidth } from '../../yard-page/lib/side-width.js';

const window = /** @type {any} */ (globalThis.window);

/** The window `width` pixels wide, as the browser tells the page once it is resized. */
function viewport(width) {
  window.happyDOM.setViewport({ width, height: 900 });
  window.dispatchEvent(new window.Event('resize'));
}

/** The sidebar's width as the page sets it, in pixels. */
function width() {
  return Number.parseFloat(document.documentElement.style.getPropertyValue('--side-width'));
}

/** @param {string} type @param {number} clientX */
function pointer(type, clientX) {
  return Object.assign(new window.Event(type), { clientX, pointerId: 1 });
}

/** @param {string} key */
function key(key) {
  return new window.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
}

/** Drags `handle` from `from` to `to` along the window's width, and lets it go. */
function drag(handle, from, to) {
  handle.dispatchEvent(pointer('pointerdown', from));
  handle.dispatchEvent(pointer('pointermove', to));
  handle.dispatchEvent(pointer('pointerup', to));
}

beforeEach(() => {
  forget();
  prefs.set('side-width', null);
  document.documentElement.style.removeProperty('--side-width');
  viewport(1280);
});

test('the sidebar first opens 248 pixels wide', () => {
  const handle = edge();
  assert.equal(FIRST, 248);
  assert.equal(width(), 248);
  assert.equal(handle.getAttribute('aria-valuenow'), '248');
});

test('dragging the sidebar’s right edge makes the sidebar wider or narrower by the length dragged', () => {
  const handle = edge();
  handle.dispatchEvent(pointer('pointerdown', 248));
  handle.dispatchEvent(pointer('pointermove', 348));
  assert.equal(width(), 348);
  handle.dispatchEvent(pointer('pointermove', 218));
  assert.equal(width(), 218);
  handle.dispatchEvent(pointer('pointerup', 218));
  assert.equal(width(), 218);
  assert.equal(handle.getAttribute('aria-valuenow'), '218');
});

test('the sidebar’s edge is reached by the keyboard, and the arrow keys left and right narrow and widen the sidebar', () => {
  const handle = edge();
  assert.equal(handle.getAttribute('role'), 'separator');
  assert.equal(handle.getAttribute('aria-orientation'), 'vertical');
  assert.equal(handle.getAttribute('tabindex'), '0');
  assert.equal(handle.getAttribute('aria-label'), 'Width of the sidebar');
  const right = key('ArrowRight');
  handle.dispatchEvent(right);
  assert.ok(right.defaultPrevented);
  assert.ok(width() > 248);
  handle.dispatchEvent(key('ArrowLeft'));
  handle.dispatchEvent(key('ArrowLeft'));
  assert.ok(width() < 248);
});

test('the sidebar is never wider than 480 pixels, and always leaves at least 120 pixels of the window beside it, also as the window changes', () => {
  assert.deepEqual([MOST, BESIDE], [480, 120]);
  assert.equal(sideWidth(2000, 1280), 480);
  assert.equal(sideWidth(2000, 500), 500 - 120);
  const handle = edge();
  drag(handle, 248, 2000);
  assert.equal(width(), 480);
  viewport(560);
  assert.equal(width(), 560 - 120);
  assert.equal(handle.getAttribute('aria-valuemax'), String(560 - 120));
  viewport(1280);
  assert.equal(width(), 480);
});

test('the sidebar is never narrower than 200 pixels, unless the window is too narrow to leave 120 pixels beside it', () => {
  assert.equal(LEAST, 200);
  assert.equal(sideWidth(40, 1280), 200);
  assert.equal(sideWidth(40, 300), 300 - 120);
  assert.equal(sideWidth(null, 300), 300 - 120);
  const handle = edge();
  drag(handle, 248, -1000);
  assert.equal(width(), 200);
  assert.equal(handle.getAttribute('aria-valuemin'), '200');
});

test('the width chosen is the sidebar’s width the next time the page opens in this browser, and a resize chooses none', () => {
  drag(edge(), 248, 328);
  viewport(400);
  assert.equal(width(), 400 - 120);
  viewport(1280);
  document.documentElement.style.removeProperty('--side-width');
  edge();
  assert.equal(width(), 328);
});
