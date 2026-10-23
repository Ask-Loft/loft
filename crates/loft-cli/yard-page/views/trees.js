// The Yard Logs the page server serves: above them, every open submission on the machine, oldest
// first, with its repository, worktree and branch; then each repository, named by the top of its
// main worktree, with each worktree's branches, the current branch's Yard Log first and each
// carried from another branch after it.

import { h } from '../lib/dom.js';
import { base, when, who } from '../lib/format.js';
import { format } from '../lib/route.js';

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
    repositories.map((repository) =>
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
              (tree.logs ?? []).map((log) => {
                const here = current && current.tree === tree.tree && current.branch === log.branch;
                return h(
                  'li',
                  { class: ['log-item', here ? 'is-current' : null, log.closed ? 'is-closed' : null, log.inherited ? 'is-inherited' : null] },
                  h(
                    'a',
                    { href: address(tree.tree, log.branch), 'aria-current': here ? 'page' : null },
                    h('span', { class: 'branch' }, log.branch),
                    log.title && log.title !== log.branch ? h('span', { class: 'title' }, log.title) : null,
                    log.waiting ? h('span', { class: 'waiting-count', title: `${log.waiting} wait on you` }, String(log.waiting)) : null,
                    log.closed ? h('span', { class: 'tag' }, 'closed') : null,
                    log.inherited ? h('span', { class: 'tag' }, 'inherited') : null,
                    log.newer ? h('span', { class: 'tag tag--warning' }, `newer format ${log.format}`) : null,
                    log.error ? h('span', { class: 'tag tag--danger' }, 'does not read') : null,
                  ),
                );
              }),
            ),
          ),
        ),
      ),
    ),
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
