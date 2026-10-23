// The height of a docked modal, the panel along the bottom of the window: half the viewport at
// first, then as the person drags its top edge or moves it with the arrow keys, kept in this
// browser once a drag or a key ends. It is never shorter than LEAST and always leaves ABOVE of the
// page in view above it; a resized viewport fits it again without choosing a height.

import * as prefs from './prefs.js';
import { separator } from './separator.js';

/** The panel's least height, and the least of the page left in view above it, in pixels. */
export const LEAST = 160;
export const ABOVE = 120;

/** How far an arrow key moves the edge, in pixels. */
const STEP = 24;

/** The panel's margin to the viewport's edge where its style tells none, in pixels. */
const MARGIN = 24;

/** The panel drawn now, and its height. */
const current = { /** @type {HTMLElement | null} */ panel: null, height: 0 };

/**
 * The panel's height for the height `chosen`, or for none, in a viewport `viewport` pixels high
 * with the panel `margin` pixels from its bottom edge.
 * @param {number | null} chosen
 * @param {number} viewport
 * @param {number} margin
 */
export function dockHeight(chosen, viewport, margin) {
  const wanted = chosen ?? viewport / 2;
  return Math.round(Math.max(LEAST, Math.min(viewport - margin - ABOVE, wanted)));
}

/** The height kept in this browser, or none. */
function kept() {
  const height = Number(prefs.get('dock-height'));
  return Number.isFinite(height) && height > 0 ? height : null;
}

/** @param {HTMLElement} panel */
function margin(panel) {
  const bottom = Number.parseFloat(window.getComputedStyle(panel).bottom);
  return Number.isFinite(bottom) ? bottom : MARGIN;
}

/**
 * Sets the panel's height to `chosen` within its bounds.
 * @param {number | null} chosen
 */
function set(chosen) {
  const panel = current.panel;
  if (!panel) return;
  const viewport = window.innerHeight;
  const room = margin(panel);
  current.height = dockHeight(chosen, viewport, room);
  document.documentElement.style.setProperty('--dock-height', `${current.height}px`);
  const handle = panel.querySelector('.dock-edge');
  handle?.setAttribute('aria-valuenow', String(current.height));
  handle?.setAttribute('aria-valuemax', String(Math.max(LEAST, viewport - room - ABOVE)));
}

/**
 * Fits `panel`, just opened, to the height kept in this browser, or to half the viewport.
 * @param {HTMLElement} panel
 */
export function fit(panel) {
  current.panel = panel;
  set(kept());
}

/** Keeps the height the person chose. */
function keep() {
  prefs.set('dock-height', String(current.height));
}

/** The panel's top edge: dragged, or moved with the arrow keys, it sets the panel's height. */
export function edge() {
  return separator({
    axis: 'y',
    step: STEP,
    attributes: { class: 'dock-edge', 'aria-label': 'Height of the panel', 'aria-valuemin': LEAST },
    read: () => current.height,
    show: set,
    keep,
  });
}

window.addEventListener('resize', () => {
  if (current.panel?.isConnected && /** @type {HTMLDialogElement} */ (current.panel).open) set(kept());
});
