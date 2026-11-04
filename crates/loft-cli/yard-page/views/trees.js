// The Yard Logs the page server serves: above them, every open submission on the machine, oldest
// first, with its repository, worktree and branch; then each repository, named by the top of its
// main worktree, with each worktree's current branch's Yard Log, and the Yard Logs carried from
// other branches in a fold of the repository's own (`inherited.js`), and beneath the branch of the
// Yard Log shown, its views. They lie in the sidebar down the window's left, headed by Loft's
// wordmark with Yard beside it, which folds to a rail that holds the button that opens it, an icon
// for each view of the Yard Log shown and the count of what waits.

import { h, svg } from '../lib/dom.js';
import { base, when, who } from '../lib/format.js';
import { format } from '../lib/route.js';
import { inheritedFold } from './inherited.js';

/**
 * The address of a branch's Yard Log.
 * @param {string} tree
 * @param {string} branch
 */
function address(tree, branch) {
  return format({ tree, branch });
}

/**
 * Every open submission of every Yard Log served, oldest first.
 * @param {any} listing
 */
export function waitingList(listing) {
  const waits = listing?.waiting ?? [];
  if (!waits.length) return null;
  return h(
    'section',
    { class: 'everywhere', 'aria-labelledby': 'everywhere-title' },
    h('h2', { id: 'everywhere-title' }, waits.length === 1 ? 'Waits on you' : `${waits.length} wait on you`),
    h(
      'ol',
      { class: 'wait-list' },
      waits.map((wait) =>
        h(
          'li',
          { class: 'wait-item' },
          h(
            'a',
            { href: address(wait.tree, wait.branch), class: 'wait-link' },
            h('span', { class: 'branch' }, wait.branch),
            h('span', { class: 'tree' }, base(wait.tree)),
            wait.main ? null : h('span', { class: 'repository' }, base(wait.repository)),
          ),
          h(
            'ul',
            { class: 'asked' },
            (wait.asked ?? wait.blocks.map((id) => ({ id }))).map((asked) =>
              h('li', {}, asked.kind ? h('span', { class: 'kind' }, asked.kind) : null, ' ', asked.text || asked.id),
            ),
          ),
          h('p', { class: 'sent' }, `Sent by ${who(wait.agent)} at `, h('time', { datetime: wait.sent?.at }, when(wait.sent?.at))),
        ),
      ),
    ),
  );
}

/** @typedef {'timeline' | 'gantt' | 'tokens'} View */
/** @typedef {{ view: View, onView: (view: View) => void }} Views */

/** The views of a Yard Log, in their order. @type {[View, string][]} */
export const VIEWS = [
  ['timeline', 'Timeline'],
  ['gantt', 'Gantt chart'],
  ['tokens', 'Tokens'],
];

/** Loft Web's icons, drawn on a square of 16: the sidebar's panel, and each view's. */
const ICONS = {
  panel: ['M2.5 3h11v10h-11zM6 3v10'],
  timeline: ['M3.5 3.5h2v2h-2zM3.5 7h2v2h-2zM3.5 10.5h2v2h-2z', 'M8 4.5h5.5M8 8h5.5M8 11.5h5.5'],
  gantt: ['M2.5 4h6M5.5 8h7.5M8.5 12h5'],
  tokens: ['M6 2.5l-1.5 11M11.5 2.5l-1.5 11M2.5 6h11M2 10h11'],
};

/** @param {keyof typeof ICONS} name */
function icon(name) {
  return svg(
    'svg',
    { class: 'side-icon', viewBox: '0 0 16 16', width: 18, height: 18, 'aria-hidden': 'true' },
    ICONS[name].map((d) => svg('path', { d })),
  );
}

/**
 * The entry of each view of the Yard Log shown, the one shown marked as current; on the rail,
 * each by its icon alone.
 * @param {Views} views
 * @param {boolean} rail
 */
function viewEntries({ view, onView }, rail) {
  const entry = ([value, name]) =>
    h(
      'button',
      {
        type: 'button',
        class: rail ? 'side-button view-entry--rail' : 'view-entry',
        dataset: { view: value },
        'aria-current': view === value ? 'page' : null,
        'aria-label': rail ? name : null,
        title: rail ? name : null,
        onclick: () => onView(value),
      },
      icon(value),
      rail ? null : name,
    );
  return rail
    ? h('nav', { class: 'rail-views', 'aria-label': 'Views' }, VIEWS.map(entry))
    : h('ul', { class: 'view-entries', 'aria-label': 'Views' }, VIEWS.map((item) => h('li', {}, entry(item))));
}

/**
 * A Yard Log of the list, at its own address, with `current` marked and, beneath it, the
 * entries of its views.
 * @param {string} tree
 * @param {any} log
 * @param {{ tree: string, branch: string } | null} current
 * @param {Views} [views]
 */
function logItem(tree, log, current, views) {
  const here = current && current.tree === tree && current.branch === log.branch;
  return h(
    'li',
    { class: ['log-item', here ? 'is-current' : null, log.closed ? 'is-closed' : null, log.inherited ? 'is-inherited' : null] },
    h(
      'a',
      { href: address(tree, log.branch), 'aria-current': here ? 'page' : null },
      h('span', { class: 'branch' }, log.branch),
      log.waiting ? h('span', { class: 'waiting-count', title: `${log.waiting} wait on you` }, String(log.waiting)) : null,
      log.title && log.title !== log.branch ? h('span', { class: 'title' }, log.title) : null,
      log.closed ? h('span', { class: 'tag' }, 'closed') : null,
      log.inherited ? h('span', { class: 'tag' }, 'inherited') : null,
      log.newer ? h('span', { class: 'tag tag--warning' }, `newer format ${log.format}`) : null,
      log.error ? h('span', { class: 'tag tag--danger' }, 'does not read') : null,
    ),
    here && views ? viewEntries(views, false) : null,
  );
}

/**
 * The list of Yard Logs, with `current` marked and the entries of its views beneath it.
 * @param {any} listing
 * @param {{ tree: string, branch: string } | null} current
 * @param {Views} [views]
 */
export function treeList(listing, current, views) {
  const repositories = listing?.repositories ?? [];
  if (!repositories.length) {
    return h(
      'section',
      { class: 'trees empty' },
      h('h2', {}, 'No Yard Log yet'),
      h('p', {}, 'A worktree is listed here once an agent in it writes its branch’s Yard Log.'),
    );
  }
  return h(
    'nav',
    { class: 'trees', 'aria-label': 'Yard Logs' },
    repositories.map((repository, index) =>
      h(
        'section',
        { class: 'repository' },
        h('h2', { title: repository.repository }, base(repository.repository)),
        repository.trees.map((tree) =>
          h(
            'div',
            { class: 'worktree' },
            tree.main ? null : h('h3', { title: tree.tree }, base(tree.tree)),
            h(
              'ul',
              { class: 'logs' },
              (tree.logs ?? []).map((log) => logItem(tree.tree, log, current, views)),
            ),
          ),
        ),
        repository.inherited ? inheritedFold(repository, index, (log) => logItem(log.tree, log, current, views)) : null,
      ),
    ),
  );
}

/**
 * The sidebar: open, its head holds the button that folds it and Loft's wordmark with Yard beside
 * it, above what waits and the list; folded, it is a rail of the button that opens it, with the
 * count of what waits on the person, and an icon for each view of the Yard Log shown.
 * @param {any} listing
 * @param {{ tree: string, branch: string } | null} current
 * @param {boolean} folded
 * @param {View} view
 * @param {{ onToggle: (folded: boolean) => void, onView: (view: View) => void }} actions
 */
export function sidebar(listing, current, folded, view, { onToggle, onView }) {
  const waiting = listing?.count ?? 0;
  const label = folded ? `Open the sidebar${waiting ? `, ${waiting} wait on you` : ''}` : 'Fold the sidebar';
  const views = { view, onView };
  const button = h(
    'button',
    {
      type: 'button',
      id: 'side-button',
      class: 'side-button',
      'aria-controls': 'side-lists',
      'aria-expanded': folded ? 'false' : 'true',
      'aria-label': label,
      title: label,
      onclick: () => onToggle(!folded),
    },
    icon('panel'),
    folded && waiting ? h('span', { class: 'waiting-count', 'aria-hidden': 'true' }, String(waiting)) : null,
  );
  const mark = h(
    'a',
    { class: 'wordmark-link', href: '#', 'aria-label': 'Loft Yard: the Yard Logs' },
    h('span', { class: 'wordmark' }, h('span', { class: 'wordmark-initial' }, 'L'), 'oft'),
    h('span', { class: 'app-name' }, 'Yard'),
  );
  return [
    h('div', { class: 'side-head' }, button, folded ? null : mark),
    folded && current ? viewEntries(views, true) : null,
    h('div', { id: 'side-lists', class: 'side-lists', hidden: folded }, waitingList(listing), treeList(listing, current, views)),
  ];
}

/**
 * What the page shows while no Yard Log is chosen: with none served, how to start a session that
 * opens one; otherwise, to choose one from the list.
 * @param {any} listing
 */
export function welcome(listing) {
  if (listing && !(listing.repositories ?? []).length) {
    return h(
      'section',
      { class: 'welcome' },
      h('h2', {}, 'Start a session'),
      h(
        'p',
        {},
        'Yard Log is kept by an agent through its Yard adapter. Start a session in a git repository, on the branch of the work, and ask it to plan or build a change: its first write opens the branch’s Yard Log, which is listed here as soon as it is written.',
      ),
      h('p', {}, h('a', { href: 'https://github.com/Ask-Loft/loft#yard-client', target: '_blank', rel: 'noopener noreferrer' }, 'Install a Yard adapter for your agent host.')),
    );
  }
  return h(
    'section',
    { class: 'welcome' },
    h('h2', {}, 'Choose a Yard Log'),
    h('p', {}, 'Each branch an agent works on keeps its Yard Log. Choose one from the list to follow it.'),
  );
}

/**
 * The page's title, with the count of what waits on the person.
 * @param {any} listing
 */
export function title(listing) {
  const waiting = listing?.count ?? 0;
  return waiting ? `(${waiting}) Yard` : 'Yard';
}
