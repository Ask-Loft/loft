// The stage of an expanded diagram, where it zooms and pans as a picture does on a trackpad: a
// pinch, or the wheel with Ctrl or ⌘ held, zooms about the pointer; the wheel or two fingers
// pan, and so does dragging; the keys `+`, `-`, `0` and the arrows, and the buttons in its
// corner, do the same. Every wheel and gesture over the stage is its own, so neither the modal
// nor the page behind it scrolls.

import { h } from './dom.js';
import { fit, pan, percent, zoomAt } from '../model/zoom.js';

/** How far a step of the keys or the buttons zooms. */
const STEP = 1.25;

/** How far an arrow key pans, in pixels. */
const NUDGE = 40;

/** The size of a wheel's line and page, for a wheel that counts in them, in pixels. */
const WHEEL_UNITS = [1, 16, 400];

/**
 * The own size of the diagram drawn in `content`: its SVG's view box.
 * @param {Element} content
 */
export function ownSize(content) {
  const drawn = content.querySelector('svg');
  const box = drawn?.getAttribute('viewBox')?.trim().split(/[\s,]+/).map(Number);
  if (box?.length === 4 && box[2] > 0 && box[3] > 0) return { width: box[2], height: box[3] };
  return { width: 800, height: 600 };
}

/**
 * A stage that zooms and pans `content`, the element a diagram is drawn into. `fit` shows the
 * whole diagram once it is drawn and the stage has its size.
 * @param {HTMLElement} content
 */
export function zoomStage(content) {
  /** @type {import('../model/zoom.js').View} */
  let view = { k: 1, x: 0, y: 0 };
  const shown = h('span', { class: 'zoom-percent', 'aria-live': 'polite' }, '100%');
  const stage = h(
    'div',
    {
      class: 'zoom-stage',
      tabindex: '0',
      'aria-label': 'The diagram. Pinch, or scroll with Ctrl or ⌘ held, to zoom; scroll or drag to pan.',
    },
    content,
  );
  const size = () => ({ width: stage.clientWidth, height: stage.clientHeight });
  const centre = () => ({ x: stage.clientWidth / 2, y: stage.clientHeight / 2 });
  /** @param {import('../model/zoom.js').View} next */
  const set = (next) => {
    view = next;
    const own = ownSize(content);
    content.style.width = `${own.width}px`;
    content.style.transform = `translate(${view.x}px, ${view.y}px) scale(${view.k})`;
    shown.textContent = `${percent(view)}%`;
  };
  // The diagram shows once it is first fitted, so that it never flashes at its own size.
  const fitted = () => {
    set(fit(size(), ownSize(content)));
    stage.classList.add('is-fitted');
  };
  /** @param {MouseEvent} event */
  const at = (event) => {
    const box = stage.getBoundingClientRect();
    return { x: event.clientX - box.left, y: event.clientY - box.top };
  };
  const button = (/** @type {string} */ label, /** @type {string} */ text, /** @type {() => void} */ act) =>
    h('button', { type: 'button', class: 'zoom-button', 'aria-label': label, title: label, onclick: act }, text);
  const tools = h(
    'div',
    { class: 'zoom-tools' },
    button('Zoom out', '−', () => set(zoomAt(view, 1 / STEP, centre()))),
    shown,
    button('Zoom in', '+', () => set(zoomAt(view, STEP, centre()))),
    button('Fit the whole diagram', 'Fit', fitted),
  );

  stage.addEventListener(
    'wheel',
    (event) => {
      event.preventDefault();
      const unit = WHEEL_UNITS[event.deltaMode] ?? 1;
      if (event.ctrlKey || event.metaKey) set(zoomAt(view, Math.exp((-event.deltaY * unit) / 100), at(event)));
      else set(pan(view, -event.deltaX * unit, -event.deltaY * unit));
    },
    { passive: false },
  );
  // Safari tells a pinch as gestures, not as wheels with Ctrl held.
  let pinched = 1;
  stage.addEventListener('gesturestart', (event) => {
    event.preventDefault();
    pinched = 1;
  });
  stage.addEventListener('gesturechange', (event) => {
    event.preventDefault();
    const gesture = /** @type {any} */ (event);
    set(zoomAt(view, gesture.scale / pinched, at(gesture)));
    pinched = gesture.scale;
  });

  /** @type {{ id: number, x: number, y: number } | null} */
  let dragging = null;
  stage.addEventListener('pointerdown', (event) => {
    if (event.button !== 0) return;
    dragging = { id: event.pointerId, x: event.clientX, y: event.clientY };
    stage.setPointerCapture?.(event.pointerId);
    stage.classList.add('is-dragging');
  });
  stage.addEventListener('pointermove', (event) => {
    if (!dragging || event.pointerId !== dragging.id) return;
    set(pan(view, event.clientX - dragging.x, event.clientY - dragging.y));
    dragging = { ...dragging, x: event.clientX, y: event.clientY };
  });
  const dropped = (/** @type {PointerEvent} */ event) => {
    if (!dragging || event.pointerId !== dragging.id) return;
    dragging = null;
    stage.classList.remove('is-dragging');
  };
  stage.addEventListener('pointerup', dropped);
  stage.addEventListener('pointercancel', dropped);

  /** @type {Record<string, () => void>} */
  const keys = {
    '+': () => set(zoomAt(view, STEP, centre())),
    '=': () => set(zoomAt(view, STEP, centre())),
    '-': () => set(zoomAt(view, 1 / STEP, centre())),
    _: () => set(zoomAt(view, 1 / STEP, centre())),
    0: fitted,
    ArrowLeft: () => set(pan(view, NUDGE, 0)),
    ArrowRight: () => set(pan(view, -NUDGE, 0)),
    ArrowUp: () => set(pan(view, 0, NUDGE)),
    ArrowDown: () => set(pan(view, 0, -NUDGE)),
  };
  stage.addEventListener('keydown', (event) => {
    const act = keys[event.key];
    if (!act || event.altKey || event.ctrlKey || event.metaKey) return;
    event.preventDefault();
    act();
  });

  return { element: h('div', { class: 'zoom' }, stage, tools), stage, fit: fitted, view: () => view };
}
