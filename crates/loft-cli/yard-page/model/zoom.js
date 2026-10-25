// The zoom of an expanded diagram: its scale against its own size, and where its top left corner
// lies in the stage that shows it. Zooming keeps the point under the pointer where it is; the
// scale stays between a tenth of the diagram's own size and eight times it.

/** The least scale, against the diagram's own size. */
export const LEAST = 0.1;

/** The greatest scale, against the diagram's own size. */
export const MOST = 8;

/** The space fitting leaves between the diagram and each edge of the stage. */
const MARGIN = 16;

/**
 * @typedef {{ k: number, x: number, y: number }} View
 * @typedef {{ width: number, height: number }} Size
 * @typedef {{ x: number, y: number }} Point
 */

/** @param {number} k */
export function clamp(k) {
  return Math.min(MOST, Math.max(LEAST, k));
}

/**
 * The view that shows the whole diagram of `size` in the stage of `stage`, centred in it.
 * @param {Size} stage
 * @param {Size} size
 * @returns {View}
 */
export function fit(stage, size) {
  const room = (/** @type {number} */ length) => Math.max(1, length - 2 * MARGIN);
  const k = clamp(Math.min(room(stage.width) / Math.max(1, size.width), room(stage.height) / Math.max(1, size.height)));
  return { k, x: (stage.width - size.width * k) / 2, y: (stage.height - size.height * k) / 2 };
}

/**
 * `view` zoomed by `factor` about `point` of the stage, which stays over the same point of the
 * diagram.
 * @param {View} view
 * @param {number} factor
 * @param {Point} point
 * @returns {View}
 */
export function zoomAt(view, factor, point) {
  const k = clamp(view.k * factor);
  const ratio = k / view.k;
  return { k, x: point.x - (point.x - view.x) * ratio, y: point.y - (point.y - view.y) * ratio };
}

/**
 * `view` moved by `dx` and `dy` across the stage.
 * @param {View} view
 * @param {number} dx
 * @param {number} dy
 * @returns {View}
 */
export function pan(view, dx, dy) {
  return { k: view.k, x: view.x + dx, y: view.y + dy };
}

/**
 * The scale of `view` as a whole percentage of the diagram's own size.
 * @param {View} view
 */
export function percent(view) {
  return Math.round(view.k * 100);
}
