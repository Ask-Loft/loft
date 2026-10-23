// The list of Yard Logs and what waits on the person across them.

import { forget } from './dom.js';

import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';

import * as prefs from '../../yard-page/lib/prefs.js';
import { parse } from '../../yard-page/lib/route.js';
import { foldButton, title, treeList, waitingList, welcome } from '../../yard-page/views/trees.js';

beforeEach(forget);

/** A listing as the page server gives it. */
function listing(more = {}) {
  return {
    repositories: [
      {
        repository: '/work/shelf',
        trees: [
          {
            tree: '/work/shelf',
            main: true,
            branch: 'feat/shelf',
            logs: [
              { branch: 'feat/shelf', inherited: false, title: 'Shelves for two readers', revision: 4, closed: false, waiting: 2 },
              { branch: 'develop', inherited: true, title: 'develop', revision: 9, closed: true, waiting: 0 },
            ],
          },
          { tree: '/work/shelf-docs', branch: 'docs', logs: [{ branch: 'docs', inherited: false, title: 'docs', closed: false, waiting: 0 }] },
        ],
      },
    ],
    waiting: [],
    count: 2,
    ...more,
  };
}

test('the Yard Logs are listed by repository, then by worktree and branch', () => {
  const list = treeList(listing(), null);
  assert.deepEqual([...list.querySelectorAll('.repository > h2')].map((each) => each.textContent), ['shelf']);
  assert.deepEqual([...list.querySelectorAll('.worktree > h3')].map((each) => each.textContent), ['shelf-docs']);
  assert.deepEqual([...list.querySelectorAll('.log-item .branch')].map((each) => each.textContent), ['feat/shelf', 'develop', 'docs']);
});

test('each repository is named by the top of its main worktree', () => {
  const list = treeList(listing(), null);
  assert.equal(list.querySelector('.repository > h2').getAttribute('title'), '/work/shelf');
});

test('each listed branch shows its worktree, its name, its title and the count of blocks that wait on the person', () => {
  const list = treeList(listing(), null);
  const item = list.querySelector('.log-item');
  assert.equal(item.querySelector('.branch').textContent, 'feat/shelf');
  assert.equal(item.querySelector('.title').textContent, 'Shelves for two readers');
  assert.equal(item.querySelector('.waiting-count').textContent, '2');
  // The repository's main worktree is named by the repository's heading, once.
  assert.equal(item.closest('.repository').querySelector('h2').getAttribute('title'), '/work/shelf');
});

test('an inherited Yard Log is listed after the current one, marked as inherited, and a closed one as closed', () => {
  const list = treeList(listing(), null);
  const items = [...list.querySelectorAll('.worktree')][0].querySelectorAll('.log-item');
  assert.ok(items[1].classList.contains('is-inherited'));
  assert.deepEqual([...items[1].querySelectorAll('.tag')].map((tag) => tag.textContent), ['closed', 'inherited']);
  assert.ok(!items[1].querySelector('.waiting-count'));
});

test('choosing a listed branch opens its Yard Log at an address of its own that names its tree and branch', () => {
  const list = treeList(listing(), null);
  const href = list.querySelector('.log-item a').getAttribute('href');
  assert.deepEqual(parse(href), { tree: '/work/shelf', branch: 'feat/shelf', block: undefined });
});

test('every open submission is shown above the list, oldest first, with its repository, worktree and branch', () => {
  const waits = waitingList({
    waiting: [
      { repository: '/work/shelf', tree: '/work/shelf', main: true, branch: 'feat/shelf', sent: { at: '2026-01-01T09:00:00Z' }, agent: { session: 's1' }, blocks: ['q1'], asked: [{ id: 'q1', kind: 'question', text: 'Which shelf?' }] },
      { repository: '/work/shelf', tree: '/work/shelf-docs', branch: 'docs', sent: { at: '2026-01-01T10:00:00Z' }, agent: { session: 's2', agent: 'a', agentType: 'Explore' }, blocks: ['p1'] },
    ],
  });
  const items = [...waits.querySelectorAll('.wait-item')];
  assert.deepEqual(items.map((item) => item.querySelector('.branch').textContent), ['feat/shelf', 'docs']);
  assert.equal(items[0].querySelector('.tree').textContent, 'shelf');
  assert.equal(items[1].querySelector('.repository').textContent, 'shelf');
  assert.match(items[0].querySelector('.asked').textContent, /Which shelf\?/);
  assert.match(items[1].querySelector('.sent').textContent, /Sent by Explore at/);
  assert.deepEqual(parse(items[1].querySelector('a').getAttribute('href')).branch, 'docs');
});

test('with no open submission anywhere, nothing is shown above the list', () => {
  assert.ok(!waitingList(listing()));
});

test('the page title gives the count of blocks that wait on the person', () => {
  assert.equal(title(listing()), '(2) Yard');
  assert.equal(title(listing({ count: 0 })), 'Yard');
});

test('a repository’s main worktree, whose folder is the repository’s own, is named once', () => {
  const list = treeList(listing(), null);
  const worktrees = [...list.querySelectorAll('.worktree')];
  assert.deepEqual(
    worktrees.map((worktree) => worktree.querySelector('h3')?.textContent ?? null),
    [null, 'shelf-docs'],
  );
  const waits = waitingList({
    ...listing(),
    waiting: [
      { repository: '/work/shelf', tree: '/work/shelf', main: true, branch: 'feat/shelf', submission: 's', sent: { at: '2026-01-01T10:00:00Z' }, agent: {}, blocks: ['q1'] },
      { repository: '/work/shelf', tree: '/work/shelf-docs', branch: 'docs', submission: 't', sent: { at: '2026-01-01T10:00:00Z' }, agent: {}, blocks: ['q2'] },
    ],
  });
  const named = [...waits.querySelectorAll('.wait-link')].map((link) => [...link.querySelectorAll('span')].map((span) => span.textContent));
  assert.deepEqual(named, [
    ['feat/shelf', 'shelf'],
    ['docs', 'shelf-docs', 'shelf'],
  ]);
});

test('with Yard Logs listed and none chosen, the page asks to choose one from the list', () => {
  const shown = welcome(listing());
  assert.equal(shown.querySelector('h2').textContent, 'Choose a Yard Log');
  assert.match(shown.textContent, /Choose one from the list/);
});

test('with no Yard Log served, the page asks for a session rather than a choice from the list', () => {
  const shown = welcome(listing({ repositories: [] }));
  assert.equal(shown.querySelector('h2').textContent, 'Start a session');
  assert.match(shown.textContent, /Claude Code/);
  assert.match(shown.textContent, /first write/);
  assert.doesNotMatch(shown.textContent, /Choose one from the list/);
});

test('with no Yard Log served, the page gives the commands that install the Loft plugin', () => {
  const shown = welcome(listing({ repositories: [] }));
  assert.deepEqual(
    [...shown.querySelectorAll('.commands code')].map((each) => each.textContent),
    ['/plugin marketplace add Ask-Loft/loft', '/plugin install loft@ask-loft'],
  );
});

test('the button at the head of the page says whether it folds the list or opens it, and asks for the other', () => {
  /** @type {boolean[]} */
  const asked = [];
  const open = foldButton(listing(), false, (folded) => asked.push(folded));
  assert.equal(open.textContent, 'Hide the list');
  assert.equal(open.getAttribute('aria-expanded'), 'true');
  assert.equal(open.getAttribute('aria-controls'), 'side');
  open.click();
  const folded = foldButton(listing(), true, (again) => asked.push(again));
  assert.match(folded.textContent ?? '', /^Show the list/);
  assert.equal(folded.getAttribute('aria-expanded'), 'false');
  folded.click();
  assert.deepEqual(asked, [true, false]);
});

test('the list is open until the person folds it, and the browser keeps the choice', () => {
  assert.equal(prefs.folded(), false);
  prefs.set('list', 'folded');
  assert.equal(prefs.folded(), true);
  assert.equal(localStorage.getItem('yard.list'), 'folded');
  prefs.set('list', 'open');
  assert.equal(prefs.folded(), false);
});

test('with the list folded, the button gives the count of blocks that wait on the person, and none while open or while none wait', () => {
  assert.equal(foldButton(listing(), true, () => {}).querySelector('.waiting-count')?.textContent, '2');
  assert.ok(!foldButton(listing(), false, () => {}).querySelector('.waiting-count'));
  assert.ok(!foldButton(listing({ count: 0 }), true, () => {}).querySelector('.waiting-count'));
});
