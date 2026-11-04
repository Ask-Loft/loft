// The Yard Logs the page server serves: above them, every open submission on the machine, oldest
// first, with its repository, worktree and branch; then each repository, named by the top of its
// main worktree, with each worktree's current branch's Yard Log, and the Yard Logs carried from
// other branches in a fold of the repository's own (`inherited.js`). They lie in the sidebar down
// the window's left, headed by Loft's wordmark with Yard beside it, which folds to a rail that
// holds the button that opens it and the count of what waits.

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

/** Loft Web's panel icon, drawn on a square of 16. */
const PANEL = 'M2.5 3h11v10h-11zM6 3v10';

function panelIcon() {
  return svg('svg', { class: 'side-icon', viewBox: '0 0 16 16', width: 18, height: 18, 'aria-hidden': 'true' }, svg('path', { d: PANEL }));
}

/**
 * A Yard Log of the list, at its own address, with `current` marked.
 * @param {string} tree
 * @param {any} log
 * @param {{ tree: string, branch: string } | null} current
 */
function logItem(tree, log, current) {
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
  );
}

/**
 * The list of Yard Logs, with `current` marked.
 * @param {any} listing
 * @param {{ tree: string, branch: string } | null} current
 */
export function treeList(listing, current) {
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
              (tree.logs ?? []).map((log) => logItem(tree.tree, log, current)),
            ),
          ),
        ),
        repository.inherited ? inheritedFold(repository, index, (log) => logItem(log.tree, log, current)) : null,
      ),
    ),
  );
}

/**
 * The sidebar: open, its head holds the button that folds it and Loft's wordmark with Yard beside
 * it, above what waits and the list; folded, it is a rail of the button that opens it, with the
 * count of what waits on the person.
 * @param {any} listing
 * @param {{ tree: string, branch: string } | null} current
 * @param {boolean} folded
 * @param {{ onToggle: (folded: boolean) => void }} actions
 */
export function sidebar(listing, current, folded, { onToggle }) {
  const waiting = listing?.count ?? 0;
  const label = folded ? `Open the sidebar${waiting ? `, ${waiting} wait on you` : ''}` : 'Fold the sidebar';
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
    panelIcon(),
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
    h('div', { id: 'side-lists', class: 'side-lists', hidden: folded }, waitingList(listing), treeList(listing, current)),
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
