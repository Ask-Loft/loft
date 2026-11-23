// The width of the open sidebar: FIRST at first, then as the person drags its right edge or moves
// it with the arrow keys, kept in this browser once a drag or a key ends. It is never wider than
// MOST and always leaves BESIDE of the window beside it, and never narrower than LEAST unless the
// window is too narrow for both; a resized window fits it again without choosing a width. The
// stylesheet reads it as `--side-width`, so a drag draws nothing of the sidebar again.

import * as prefs from './prefs.js';
import { separator } from './separator.js';

/** The sidebar's first width, its least and its most, and the least of the window it leaves beside it, in pixels. */
export const FIRST = 248;
export const LEAST = 200;
export const MOST = 480;
export const BESIDE = 120;

/** How far an arrow key moves the edge, in pixels. */
const STEP = 16;

/** The edge drawn now, and the sidebar's width. */
const current = { /** @type {HTMLElement | null} */ handle: null, width: FIRST };

/**
 * The sidebar's width for the width `chosen`, or for none, in a window `viewport` pixels wide.
 * @param {number | null} chosen
 * @param {number} viewport
 */
export function sideWidth(chosen, viewport) {
  const most = Math.max(0, Math.min(MOST, viewport - BESIDE));
  const least = Math.min(LEAST, most);
  return Math.round(Math.max(least, Math.min(most, chosen ?? FIRST)));
}

/** The width kept in this browser, or none. */
function kept() {
  const width = Number(prefs.get('side-width'));
  return Number.isFinite(width) && width > 0 ? width : null;
}

/**
 * Sets the sidebar's width to `chosen` within its bounds.
 * @param {number | null} chosen
 */
function set(chosen) {
  const viewport = window.innerWidth;
  current.width = sideWidth(chosen, viewport);
  document.documentElement.style.setProperty('--side-width', `${current.width}px`);
  const handle = current.handle;
  handle?.setAttribute('aria-valuenow', String(current.width));
  handle?.setAttribute('aria-valuemin', String(sideWidth(0, viewport)));
  handle?.setAttribute('aria-valuemax', String(sideWidth(Number.POSITIVE_INFINITY, viewport)));
}

/** Keeps the width the person chose. */
function keep() {
  prefs.set('side-width', String(current.width));
}

/**
 * The sidebar's right edge, fitted to the width kept in this browser: dragged, or moved with the
 * arrow keys, it sets the sidebar's width.
 */
export function edge() {
  current.handle = separator({
    axis: 'x',
    step: STEP,
    attributes: { class: 'side-edge', 'aria-label': 'Width of the sidebar', 'aria-controls': 'side' },
    read: () => current.width,
    show: set,
    keep,
  });
  set(kept());
  return current.handle;
}

window.addEventListener('resize', () => {
  if (current.handle) set(kept());
});
