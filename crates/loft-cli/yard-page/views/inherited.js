// A repository's inherited Yard Logs, in a fold of their own under it: folded unless the person
// opened it, which the browser keeps; opened, they are fetched from the page server 50 at a time,
// the one closed last first, filtered by branch or title. The listing gives only their count, so
// the menu holds none of them until the person asks.

import * as api from '../lib/api.js';
import { h } from '../lib/dom.js';
import * as prefs from '../lib/prefs.js';

/** How many inherited Yard Logs the page server gives at a time. */
export const PAGE = 50;

/** How long the filter waits for the person to stop typing, in milliseconds. */
const PAUSE = 200;

/**
 * What the page holds of each repository's inherited Yard Logs, by the repository, so that a
 * menu drawn again shows them as they were: the filter, those fetched and how many match.
 * @typedef {{ filter: string, logs: any[], total: number | null, asked: number }} Held
 * @type {Map<string, Held>}
 */
const held = new Map();

/** Forgets what is held, as a page loaded again does; for the tests. */
export function forget() {
  held.clear();
}

/** @param {string} repository */
function heldOf(repository) {
  let found = held.get(repository);
  if (!found) {
    found = { filter: '', logs: [], total: null, asked: 0 };
    held.set(repository, found);
  }
  return found;
}

/** Whether the person opened `repository`'s fold, as the browser keeps it. @param {string} repository */
export function isOpen(repository) {
  return Boolean(prefs.getJson('inherited', {})[repository]);
}

/** @param {string} repository @param {boolean} open */
function keepOpen(repository, open) {
  const kept = { ...prefs.getJson('inherited', {}) };
  if (open) kept[repository] = true;
  else delete kept[repository];
  prefs.setJson('inherited', Object.keys(kept).length ? kept : null);
}

/**
 * The fold of `repository`'s inherited Yard Logs, each drawn by `item`; `index` names its filter
 * so that the focus stays in it when the menu is drawn again.
 * @param {{ repository: string, inherited: number }} repository
 * @param {number} index
 * @param {(log: any) => HTMLElement} item
 */
export function inheritedFold(repository, index, item) {
  const name = repository.repository;
  const state = heldOf(name);
  const list = h('ul', { class: 'logs inherited-logs' });
  const more = h('button', { type: 'button', class: 'more-button', hidden: true, onclick: () => void fetchFrom(state.logs.length) }, `Show ${PAGE} more`);
  const words = h('p', { class: 'inherited-words', 'aria-live': 'polite' });
  const draw = () => {
    list.replaceChildren(...state.logs.map(item));
    more.hidden = state.total === null || state.logs.length >= state.total;
    words.textContent =
      state.total === null ? 'Loading…' : state.filter && !state.total ? 'No inherited Yard Log matches the filter.' : '';
  };
  /** Fetches the page from `from`, after those held; from 0, in place of them. @param {number} from */
  const fetchFrom = async (from) => {
    const asked = (state.asked += 1);
    const reply = await api.inherited(name, state.filter, from);
    if (asked !== state.asked || reply.status !== 200) return;
    state.total = reply.body.total ?? 0;
    state.logs = from ? [...state.logs, ...(reply.body.logs ?? [])] : reply.body.logs ?? [];
    draw();
  };
  /** Fetches again as many as are held, a page at a time, so a menu drawn again keeps them. */
  const again = async () => {
    const wanted = Math.max(PAGE, state.logs.length);
    await fetchFrom(0);
    while (state.total !== null && state.logs.length < Math.min(wanted, state.total)) {
      const before = state.logs.length;
      await fetchFrom(before);
      if (state.logs.length === before) break;
    }
  };
  let pause = /** @type {ReturnType<typeof setTimeout> | null} */ (null);
  const filter = h('input', {
    type: 'search',
    id: `inherited-filter-${index}`,
    class: 'inherited-filter',
    placeholder: 'Filter by branch or title',
    'aria-label': `Filter the inherited Yard Logs of ${name}`,
    value: state.filter,
    oninput: (/** @type {Event} */ event) => {
      state.filter = /** @type {HTMLInputElement} */ (event.currentTarget).value;
      if (pause) clearTimeout(pause);
      pause = setTimeout(() => void fetchFrom(0), PAUSE);
    },
  });
  const open = isOpen(name);
  const fold = h(
    'details',
    {
      class: 'inherited',
      open,
      ontoggle: (/** @type {Event} */ event) => {
        const now = /** @type {HTMLDetailsElement} */ (event.currentTarget).open;
        if (now === isOpen(name)) return;
        keepOpen(name, now);
        if (now) void again();
      },
    },
    h('summary', {}, 'Inherited ', h('span', { class: 'inherited-count' }, `(${repository.inherited})`)),
    filter,
    words,
    list,
    more,
  );
  draw();
  if (open) void again();
  return fold;
}
