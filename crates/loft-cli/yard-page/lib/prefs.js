// What the page keeps in this browser, and only here: the order of the stages, the view chosen,
// whether the list of Yard Logs is folded, the revision of each Yard Log's last visit, and the answers chosen and not sent. Storage may be
// missing or refuse, as in a private window; the page then works with what it holds in memory.

const PREFIX = 'yard.';

/** @type {Map<string, string>} */
const memory = new Map();

/** @returns {Storage | null} */
function storage() {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

/**
 * @param {string} name
 * @returns {string | null}
 */
export function get(name) {
  try {
    const kept = storage()?.getItem(PREFIX + name);
    if (kept !== null && kept !== undefined) return kept;
  } catch {
    // Storage refused; fall back to memory.
  }
  return memory.get(name) ?? null;
}

/**
 * @param {string} name
 * @param {string | null} value
 */
export function set(name, value) {
  if (value === null) memory.delete(name);
  else memory.set(name, value);
  try {
    if (value === null) storage()?.removeItem(PREFIX + name);
    else storage()?.setItem(PREFIX + name, value);
  } catch {
    // Storage refused; memory holds it for this visit.
  }
}

/**
 * A kept JSON value, or `fallback`.
 * @template T
 * @param {string} name
 * @param {T} fallback
 * @returns {T}
 */
export function getJson(name, fallback) {
  const kept = get(name);
  if (kept === null) return fallback;
  try {
    return JSON.parse(kept);
  } catch {
    return fallback;
  }
}

/**
 * @param {string} name
 * @param {unknown} value
 */
export function setJson(name, value) {
  set(name, value === null || value === undefined ? null : JSON.stringify(value));
}

/** The order of the stages: `newest` first unless the person chose `oldest`. */
export function order() {
  return get('order') === 'oldest' ? 'oldest' : 'newest';
}

/** The view of Yard Log: the `timeline` unless the person chose the `gantt` chart. */
export function view() {
  return get('view') === 'gantt' ? 'gantt' : 'timeline';
}

/** Whether the list of Yard Logs is folded: open unless the person folded it. */
export function folded() {
  return get('list') === 'folded';
}
