// What the page keeps of each Yard Log in this browser: each fold the person opened or closed, by
// its block's key and a name (`<block key>/<name>`), the Feature Group open in each spec-change
// view, and where its Gantt chart was scrolled. A fold is kept only while it differs from its
// default, and a stage's only while its status is the one it was set under; what a Yard Log no
// longer holds is forgotten as it is drawn, and so are all but the 30 Yard Logs opened last. A
// write waits a moment, so that a scroll does not rewrite storage at every frame, and is made at
// once when the page is hidden; it takes up what another tab wrote of the other Yard Logs.

import { h } from './dom.js';
import { getJson, setJson } from './prefs.js';

const NAME = 'kept';
/** How many Yard Logs the browser keeps. */
const LOGS = 30;
/** How long a write waits, in milliseconds. */
const WAIT = 150;

/**
 * @typedef {{ value: any, basis?: string }} Entry
 * @typedef {{ at: number, folds: Record<string, Entry>, gantt?: { left: number, top: number } }} Kept
 */

/** @type {Record<string, Kept> | null} */
let all = null;
/** The key of the Yard Log drawn. */
let current = '';
/** What is kept while no Yard Log is drawn, in memory only. @type {Kept} */
let loose = { at: 0, folds: {} };
/** The Yard Logs changed since the last write. */
const dirty = new Set();
/** @type {ReturnType<typeof setTimeout> | null} */
let pending = null;
let listening = false;

/** @returns {Record<string, Kept>} */
function store() {
  all ??= getJson(NAME, {});
  return /** @type {Record<string, Kept>} */ (all);
}

/** @returns {Kept} */
function mine() {
  if (!current) return loose;
  const logs = store();
  logs[current] ??= { at: 0, folds: {} };
  return logs[current];
}

/**
 * All but the `LOGS` Yard Logs opened last, which the browser forgets.
 * @param {Record<string, Kept>} logs
 */
function bound(logs) {
  const newest = Object.entries(logs).sort(([, a], [, b]) => (b.at ?? 0) - (a.at ?? 0));
  for (const [key] of newest.slice(LOGS)) delete logs[key];
}

function save() {
  if (!current) return;
  dirty.add(current);
  if (!listening && typeof globalThis.addEventListener === 'function') {
    listening = true;
    globalThis.addEventListener('pagehide', flush);
  }
  if (!pending) pending = setTimeout(flush, WAIT);
}

/** Writes what changed now, over what another tab wrote of the other Yard Logs. */
export function flush() {
  if (pending) clearTimeout(pending);
  pending = null;
  if (!dirty.size || !all) return;
  /** @type {Record<string, Kept>} */
  const fresh = getJson(NAME, {});
  for (const key of dirty) {
    if (all[key]) fresh[key] = all[key];
    else delete fresh[key];
  }
  bound(fresh);
  dirty.clear();
  setJson(NAME, fresh);
  all = fresh;
}

/** Forgets what is held in memory, as a page loaded again does; for the tests. */
export function reset() {
  if (pending) clearTimeout(pending);
  pending = null;
  all = null;
  current = '';
  loose = { at: 0, folds: {} };
  dirty.clear();
}

/**
 * Makes `log` the Yard Log drawn: the newest opened, holding no fold of a block it no longer holds.
 * @param {any} log
 */
export function begin(log) {
  current = log?.yardLog?.key ?? '';
  if (!current) {
    loose = { at: 0, folds: {} };
    return;
  }
  const logs = store();
  const kept = mine();
  const others = Object.entries(logs).filter(([key]) => key !== current);
  const newest = Math.max(0, ...others.map(([, each]) => each.at ?? 0));
  if (kept.at <= newest) kept.at = newest + 1;
  const keys = new Set();
  for (const stage of log.stages ?? []) {
    keys.add(stage.key);
    for (const block of stage.blocks ?? []) keys.add(block.key);
  }
  for (const path of Object.keys(kept.folds)) {
    if (!keys.has(path.split('/')[0])) delete kept.folds[path];
  }
  bound(logs);
  save();
}

/**
 * What the person chose at `path`, or `fallback` where they chose nothing under `basis`.
 * @template T
 * @param {string} path
 * @param {T} fallback
 * @param {string} [basis]
 * @returns {T}
 */
export function get(path, fallback, basis = '') {
  const entry = mine().folds[path];
  if (!entry || (entry.basis ?? '') !== basis) return fallback;
  return entry.value;
}

/**
 * Keeps `value` at `path`, under `basis`, or forgets it where it is `fallback`.
 * @param {string} path
 * @param {unknown} value
 * @param {unknown} fallback
 * @param {string} [basis]
 */
export function set(path, value, fallback, basis = '') {
  const folds = mine().folds;
  if (JSON.stringify(value) === JSON.stringify(fallback)) delete folds[path];
  else folds[path] = basis ? { value, basis } : { value };
  save();
}

/**
 * A `<details>` open as the person left it at `path`, or as `open` says, keeping each toggle.
 * Without a path it is kept nowhere.
 * @param {{ path: string | null, open: boolean, basis?: string }} fold
 * @param {Record<string, any>} attributes
 * @param {...any} children
 */
export function details(fold, attributes, ...children) {
  const { path, open, basis = '' } = fold;
  return h(
    'details',
    {
      ...attributes,
      open: path ? get(path, open, basis) : open,
      ontoggle: path
        ? (/** @type {Event} */ event) => set(path, /** @type {HTMLDetailsElement} */ (event.currentTarget).open, open, basis)
        : null,
    },
    ...children,
  );
}

/** The folds of the trace view, each folded unless the person opened it. */
export const folds = {
  /** @param {string | null} path */
  open: (path) => Boolean(path && get(path, false)),
  /** @param {string | null} path @param {boolean} open */
  set: (path, open) => {
    if (path) set(path, open, false);
  },
};

/** Where the Gantt chart of the Yard Log drawn was scrolled, or `null` before it was shown. */
export function scroll() {
  return mine().gantt ?? null;
}

/** @param {{ left: number, top: number }} value */
export function setScroll(value) {
  mine().gantt = value;
  save();
}
