// The Yard Logs the page server serves: above them, every open submission on the machine, oldest
// first, with its repository, worktree and branch; then each repository, named by the top of its
// main worktree, with each worktree's current branch's Yard Log, and the Yard Logs carried from
// other branches in a fold of the repository's own (`inherited.js`). They lie in the menu down the window's left, which folds
// to a strip that holds only the button that opens it.

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
      log.title && log.title !== log.branch ? h('span', { class: 'title' }, log.title) : null,
      log.waiting ? h('span', { class: 'waiting-count', title: `${log.waiting} wait on you` }, String(log.waiting)) : null,
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

/** Three lines: the button that opens the folded menu. */
function threeLines() {
  return svg(
    'svg',
    { class: 'menu-icon', viewBox: '0 0 20 20', width: 20, height: 20, 'aria-hidden': 'true' },
    [5, 10, 15].map((y) => svg('path', { d: `M3 ${y}h14` })),
  );
}

/** Two chevrons pointing left: the button that folds the open menu. */
function foldArrows() {
  return svg(
    'svg',
    { class: 'menu-icon', viewBox: '0 0 20 20', width: 20, height: 20, 'aria-hidden': 'true' },
    svg('path', { d: 'M10 5l-5 5 5 5' }),
    svg('path', { d: 'M16 5l-5 5 5 5' }),
  );
}

/**
 * The menu of Yard Logs: open, its head names it beside the button that folds it, above what
 * waits and the list; folded, its head holds only the button that opens it, with the count of
 * what waits on the person, which the folded list no longer shows.
 * @param {any} listing
 * @param {{ tree: string, branch: string } | null} current
 * @param {boolean} folded
 * @param {(folded: boolean) => void} onToggle
 */
export function menu(listing, current, folded, onToggle) {
  const waiting = listing?.count ?? 0;
  const label = folded ? `Open the menu of Yard Logs${waiting ? `, ${waiting} wait on you` : ''}` : 'Fold the menu of Yard Logs';
  const button = h(
    'button',
    {
      type: 'button',
      id: 'menu-button',
      class: 'menu-button',
      'aria-controls': 'menu-lists',
      'aria-expanded': folded ? 'false' : 'true',
      'aria-label': label,
      title: label,
      onclick: () => onToggle(!folded),
    },
    folded ? threeLines() : foldArrows(),
    folded && waiting ? h('span', { class: 'waiting-count', 'aria-hidden': 'true' }, String(waiting)) : null,
  );
  return [
    h('div', { class: 'menu-head' }, folded ? null : h('h2', { class: 'menu-title' }, 'Yard Logs'), button),
    h('div', { id: 'menu-lists', class: 'menu-lists', hidden: folded }, waitingList(listing), treeList(listing, current)),
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
        'Yard Log is kept by an agent in Claude Code with the Loft plugin. Start a session in a git repository, on the branch of the work, and ask it to plan or build a change: its first write opens the branch’s Yard Log, which is listed here as soon as it is written.',
      ),
      h('p', {}, 'To install the plugin, run these in Claude Code:'),
      h(
        'ol',
        { class: 'commands' },
        h('li', {}, h('code', {}, '/plugin marketplace add Ask-Loft/loft')),
        h('li', {}, h('code', {}, '/plugin install loft@ask-loft')),
      ),
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
