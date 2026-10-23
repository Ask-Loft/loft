// The Yard page: the list of Yard Logs the page server serves and, at its own address, one
// branch's Yard Log, kept up with every change through the page server's stream.

import * as api from './lib/api.js';
import { h, replace } from './lib/dom.js';
import { drawAll } from './lib/diagrams.js';
import * as prefs from './lib/prefs.js';
import { format, parse } from './lib/route.js';
import { wantedBy } from './model/change.js';
import { settle } from './views/gantt.js';
import { logView, unreadable } from './views/log.js';
import { foldButton, title, treeList, waitingList, welcome } from './views/trees.js';

const state = {
  /** @type {any} */ listing: null,
  /** @type {any} */ log: null,
  /** @type {number} */ logStatus: 0,
  /** @type {string | null} */ version: null,
  /** The revision each Yard Log was at when this visit began, by its key. @type {Map<string, number | null>} */
  visits: new Map(),
  notice: '',
};

/** The revision of the last visit to a Yard Log, as this visit began; recorded once. */
function lastVisit(log) {
  const key = log?.yardLog?.key;
  if (!key) return null;
  if (!state.visits.has(key)) {
    const kept = prefs.getJson('visits', {});
    state.visits.set(key, typeof kept[key] === 'number' ? kept[key] : null);
  }
  const visits = prefs.getJson('visits', {});
  prefs.setJson('visits', { ...visits, [key]: log.revision });
  return state.visits.get(key) ?? null;
}

/**
 * Loads what `wanted` names, the listing or the Yard Log shown, and draws the page again: on a
 * change only where either changed, and always when the page is opened or its address changes.
 * @param {{ listing: boolean, log: boolean }} wanted
 * @param {boolean} [always]
 */
async function load(wanted = { listing: true, log: true }, always = true) {
  const route = parse(location.hash);
  const [listing, log] = await Promise.all([
    wanted.listing ? api.trees() : Promise.resolve(null),
    wanted.log && route ? api.log(route.tree, route.branch) : Promise.resolve(null),
  ]);
  const before = JSON.stringify([state.listing, state.log, state.logStatus]);
  if (listing) state.listing = listing.body;
  if (wanted.log) {
    state.log = log?.body ?? null;
    state.logStatus = log?.status ?? 0;
  }
  if (always || JSON.stringify([state.listing, state.log, state.logStatus]) !== before) render();
}

let pending = null;
/** @type {{ listing: boolean, log: boolean }} */
let wanting = { listing: false, log: false };
/**
 * Loads again soon, once, however many changes arrive together: what each of them asks for.
 * @param {import('./model/change.js').Change} [change]
 */
function refresh(change = { all: true }) {
  const wanted = wantedBy(change, parse(location.hash));
  wanting = { listing: wanting.listing || wanted.listing, log: wanting.log || wanted.log };
  if (pending || !(wanting.listing || wanting.log)) return;
  pending = setTimeout(() => {
    pending = null;
    const loading = wanting;
    wanting = { listing: false, log: false };
    void load(loading, false);
  }, 120);
}

/** @param {string} text */
function notify(text) {
  state.notice = text;
  const region = document.getElementById('notice');
  if (region) region.textContent = text;
}

function render() {
  const route = parse(location.hash);
  document.title = title(state.listing);
  const focused = /** @type {HTMLInputElement | null} */ (document.activeElement);
  const focus = focused?.id ? { id: focused.id, start: focused.selectionStart, end: focused.selectionEnd } : null;
  const main = document.getElementById('log');
  const side = document.getElementById('side');
  if (!main || !side) return;
  replace(side, waitingList(state.listing), treeList(state.listing, route));
  const fold = document.getElementById('fold');
  if (fold) {
    replace(
      fold,
      foldButton(state.listing, prefs.folded(), (folded) => {
        prefs.set('list', folded ? 'folded' : 'open');
        render();
      }),
    );
  }
  document.body.classList.toggle('is-folded', prefs.folded());
  if (!route) {
    replace(main, welcome(state.listing));
  } else if (state.logStatus !== 200) {
    replace(main, unreadable(state.log));
  } else {
    const now = Date.now();
    replace(
      main,
      h('p', { id: 'notice', class: 'notice', role: 'status', 'aria-live': 'polite' }, state.notice),
      logView(state.log, {
        tree: route.tree,
        branch: route.branch,
        last: lastVisit(state.log),
        now,
        width: main.clientWidth || 960,
        refresh,
        notify,
        onView: render,
        onPick: (id) => {
          prefs.set('view', 'timeline');
          location.hash = format({ ...route, block: id });
        },
      }),
    );
    drawAll(main);
    settle(main);
    if (route.block) reveal(route.block);
  }
  if (focus) {
    const again = /** @type {HTMLInputElement | null} */ (document.getElementById(focus.id));
    if (again) {
      again.focus();
      if (typeof focus.start === 'number' && 'setSelectionRange' in again) {
        try {
          again.setSelectionRange(focus.start, focus.end ?? focus.start);
        } catch {
          // An input that holds no text selection.
        }
      }
    }
  }
}

/** Opens and shows the block `id`. @param {string} id */
function reveal(id) {
  const target = document.getElementById(`block-${id}`);
  if (!target) return;
  for (let at = target; at; at = at.parentElement) {
    if (at instanceof HTMLDetailsElement) at.open = true;
  }
  target.classList.add('is-picked');
  target.scrollIntoView({ block: 'center' });
}

function start() {
  window.addEventListener('hashchange', () => {
    state.notice = '';
    void load();
  });
  let width = 0;
  new ResizeObserver(([entry]) => {
    const next = Math.round(entry.contentRect.width);
    if (next !== width && prefs.view() === 'gantt') render();
    width = next;
  }).observe(document.getElementById('log') ?? document.body);
  api.follow({
    onHello: (protocol) => {
      // A page server of another version took over: the page takes up its files. What the person
      // chose and did not send is kept in the browser, so it outlives the reload.
      if (state.version && protocol.loft && protocol.loft !== state.version) {
        location.reload();
        return;
      }
      state.version ??= protocol.loft ?? null;
      refresh();
    },
    onChange: refresh,
    onState: (open) => {
      document.body.classList.toggle('is-offline', !open);
      const words = document.querySelector('.connection-words');
      if (words) words.textContent = open ? 'Live' : 'Reconnecting to the page server…';
    },
  });
  void load();
}

if (typeof window !== 'undefined' && document.getElementById('log')) start();
