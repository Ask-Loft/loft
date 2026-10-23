// A separator the person drags, or moves with the arrow keys, to set the size of what it edges:
// the dock's top edge and the edge of the Gantt chart's column of names.

import { h } from './dom.js';

/**
 * A separator along `axis`: dragged, `show` sets the size it makes from the size `read` gave as
 * the drag began; let go, or moved `step` pixels by an arrow key, `keep` keeps it. Along `x` the
 * size grows to the right; along `y` it grows upward, as an edge above what it sizes does.
 * @param {{
 *   axis: 'x' | 'y',
 *   step: number,
 *   attributes: Record<string, any>,
 *   read: () => number,
 *   show: (size: number) => void,
 *   keep: () => void,
 * }} options
 */
export function separator({ axis, step, attributes, read, show, keep }) {
  let from = /** @type {number | null} */ (null);
  let grabbed = 0;
  const along = (/** @type {any} */ event) => (axis === 'x' ? event.clientX - (from ?? 0) : (from ?? 0) - event.clientY);
  /** @type {Record<string, number>} */
  const keys = axis === 'x' ? { ArrowLeft: -step, ArrowRight: step } : { ArrowUp: step, ArrowDown: -step };
  const handle = h('div', { role: 'separator', tabindex: 0, 'aria-orientation': axis === 'x' ? 'vertical' : 'horizontal', ...attributes });
  handle.addEventListener('pointerdown', (/** @type {any} */ event) => {
    from = axis === 'x' ? event.clientX : event.clientY;
    grabbed = read();
    handle.setPointerCapture?.(event.pointerId);
    event.preventDefault();
  });
  handle.addEventListener('pointermove', (/** @type {any} */ event) => {
    if (from !== null) show(grabbed + along(event));
  });
  handle.addEventListener('pointerup', () => {
    if (from === null) return;
    from = null;
    keep();
  });
  handle.addEventListener('keydown', (/** @type {any} */ event) => {
    const moved = keys[event.key] ?? 0;
    if (!moved) return;
    event.preventDefault();
    show(read() + moved);
    keep();
  });
  return handle;
}
