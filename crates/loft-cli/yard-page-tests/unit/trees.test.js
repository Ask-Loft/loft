// The list of Yard Logs and what waits on the person across them.

import { forget } from './dom.js';

import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';

import * as prefs from '../../yard-page/lib/prefs.js';
import { parse } from '../../yard-page/lib/route.js';
import * as inherited from '../../yard-page/views/inherited.js';
import { sidebar, title, treeList, waitingList, welcome } from '../../yard-page/views/trees.js';

beforeEach(() => {
  forget();
  prefs.set('inherited', null);
  inherited.forget();
});

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
            logs: [{ branch: 'feat/shelf', inherited: false, title: 'Shelves for two readers', revision: 4, closed: false, waiting: 2 }],
          },
          { tree: '/work/shelf-docs', branch: 'docs', logs: [{ branch: 'docs', inherited: false, title: 'docs', closed: false, waiting: 0 }] },
        ],
        inherited: 51,
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
  assert.deepEqual([...list.querySelectorAll('.log-item .branch')].map((each) => each.textContent), ['feat/shelf', 'docs']);
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

test('choosing a listed branch opens its Yard Log at an address of its own that names its tree and branch', () => {
  const list = treeList(listing(), null);
  const href = list.querySelector('.log-item a').getAttribute('href');
  assert.deepEqual(parse(href), { tree: '/work/shelf', branch: 'feat/shelf', stage: undefined, block: undefined });
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
  assert.match(shown.textContent, /agent through its Yard adapter/);
  assert.match(shown.textContent, /first write/);
  assert.doesNotMatch(shown.textContent, /Choose one from the list/);
});

test('with no Yard Log served, the page links adapter installation instructions', () => {
  const shown = welcome(listing({ repositories: [] }));
  assert.match(shown.querySelector('a').textContent, /Install a Yard adapter/);
  assert.equal(shown.querySelector('a').getAttribute('href'), 'https://github.com/Ask-Loft/loft#yard-client');
});

/** The sidebar drawn from `shown`, folded or open, for the Yard Log `current` in the view `view`. */
function side(shown, { folded = false, current = null, view = 'timeline', onToggle = () => {}, onView = () => {} } = {}) {
  return sidebar(shown, current, folded, view, { onToggle, onView }).filter(Boolean);
}

const SHELF = { tree: '/work/shelf', branch: 'feat/shelf' };

test('open, the sidebar is headed by the button that folds it and the wordmark, above the lists', () => {
  /** @type {boolean[]} */
  const asked = [];
  const [head, lists] = side(listing(), { onToggle: (folded) => asked.push(folded) });
  const button = head.querySelector('button');
  assert.equal(button?.getAttribute('aria-label'), 'Fold the sidebar');
  assert.equal(button?.getAttribute('aria-expanded'), 'true');
  assert.equal(button?.getAttribute('aria-controls'), lists.id);
  assert.ok(button?.compareDocumentPosition(head.querySelector('.wordmark-link')) & Node.DOCUMENT_POSITION_FOLLOWING, 'the wordmark follows the button');
  assert.ok(!lists.hidden);
  assert.ok(lists.querySelector('nav.trees'));
  button?.click();
  assert.deepEqual(asked, [true]);
});

test('the sidebar is headed by Loft’s wordmark, its L apart from the rest of the name, with Yard beside it', () => {
  const [head] = side(listing());
  const mark = head.querySelector('.wordmark-link');
  const word = mark?.querySelector('.wordmark');
  assert.equal(word?.textContent, 'Loft');
  assert.equal(word?.querySelector('.wordmark-initial')?.textContent, 'L');
  assert.equal(word?.childNodes.length, 2, 'the L and the rest of the name, with no space between them');
  assert.equal(mark?.querySelector('.app-name')?.textContent, 'Yard');
  assert.ok(word?.compareDocumentPosition(mark.querySelector('.app-name')) & Node.DOCUMENT_POSITION_FOLLOWING);
  assert.equal(head.textContent, 'LoftYard', 'the head names no other mark');
});

test('folded, the sidebar is a rail of the button that opens it and an icon for each view of the Yard Log shown', () => {
  /** @type {boolean[]} */
  const asked = [];
  const [head, rail, lists] = side(listing({ count: 0 }), { folded: true, current: SHELF, view: 'gantt', onToggle: (folded) => asked.push(folded) });
  assert.ok(!head.querySelector('.wordmark-link'));
  const button = head.querySelector('button');
  assert.equal(button?.getAttribute('aria-label'), 'Open the sidebar');
  assert.equal(button?.getAttribute('aria-expanded'), 'false');
  assert.ok(button?.querySelector('svg'));
  const views = [...rail.querySelectorAll('button')].map((entry) => [entry.getAttribute('aria-label'), entry.getAttribute('aria-current')]);
  assert.deepEqual(views, [
    ['Timeline', null],
    ['Gantt chart', 'page'],
    ['Tokens', null],
  ]);
  assert.ok(rail.querySelectorAll('svg').length === 3);
  assert.ok(lists.hidden);
  button?.click();
  assert.deepEqual(asked, [false]);
  assert.equal(side(listing(), { folded: true }).length, 2, 'with no Yard Log shown, the rail holds no view');
});

test('the sidebar is open until the person folds it, and the browser keeps the choice', () => {
  assert.equal(prefs.folded(), false);
  prefs.set('list', 'folded');
  assert.equal(prefs.folded(), true);
  assert.equal(localStorage.getItem('yard.list'), 'folded');
  prefs.set('list', 'open');
  assert.equal(prefs.folded(), false);
});

test('with the sidebar folded, the rail gives the count of blocks that wait on the person, and none while open or while none wait', () => {
  const [folded] = side(listing(), { folded: true });
  assert.equal(folded.querySelector('.waiting-count')?.textContent, '2');
  assert.equal(folded.querySelector('button')?.getAttribute('aria-label'), 'Open the sidebar, 2 wait on you');
  assert.ok(!side(listing())[0].querySelector('.waiting-count'));
  assert.ok(!side(listing({ count: 0 }), { folded: true })[0].querySelector('.waiting-count'));
});

test('beneath the branch of the Yard Log shown lie the entries of its views, the one shown marked as current', () => {
  /** @type {string[]} */
  const chosen = [];
  const [, lists] = side(listing(), { current: SHELF, view: 'tokens', onView: (view) => chosen.push(view) });
  const item = lists.querySelector('.log-item.is-current');
  assert.equal(item?.querySelector('a')?.getAttribute('aria-current'), 'page');
  const entries = [...item.querySelectorAll('.view-entries .view-entry')];
  assert.deepEqual(entries.map((entry) => entry.textContent), ['Timeline', 'Gantt chart', 'Tokens']);
  assert.deepEqual(entries.map((entry) => entry.getAttribute('aria-current')), [null, null, 'page']);
  assert.ok(entries.every((entry) => entry.querySelector('svg')), 'each entry has its icon');
  entries[1].click();
  assert.deepEqual(chosen, ['gantt']);
  assert.equal(lists.querySelectorAll('.view-entries').length, 1, 'no other listed branch shows the entries of its views');
});

/** The inherited Yard Logs the page server holds in these tests: 51 of them, `old-00` closed last. */
const HELD = Array.from({ length: 51 }, (_, n) => ({
  branch: `old-${String(n).padStart(2, '0')}`,
  title: n === 7 ? 'The reading lamp' : `Old work ${n}`,
  inherited: true,
  closed: true,
  tree: '/work/shelf',
}));

/** A page server stub that answers `/api/inherited` from `HELD`; gives the queries it was asked. */
function serving() {
  /** @type {URLSearchParams[]} */
  const asked = [];
  globalThis.fetch = /** @type {any} */ (
    async (/** @type {string} */ url) => {
      const query = new URL(url, 'http://127.0.0.1:7411').searchParams;
      asked.push(query);
      const filter = (query.get('filter') ?? '').toLowerCase();
      const from = Number(query.get('from') ?? 0);
      const matching = HELD.filter((log) => !filter || `${log.branch} ${log.title}`.toLowerCase().includes(filter));
      const body = { total: matching.length, from, logs: matching.slice(from, from + 50) };
      return { status: 200, text: async () => JSON.stringify(body) };
    }
  );
  return asked;
}

/** Lets the stubbed fetches and the draws after them run. */
const settled = (ms = 0) => new Promise((resolve) => setTimeout(resolve, ms));

/** The fold of the inherited Yard Logs in `list`. */
function foldOf(list) {
  return /** @type {HTMLDetailsElement} */ (list.querySelector('details.inherited'));
}

test('each tree lists only its current branch’s Yard Log, and the repository folds the inherited ones beneath their count', async () => {
  const asked = serving();
  const list = treeList(listing(), null);
  const fold = foldOf(list);
  assert.equal(fold.querySelector('summary')?.textContent, 'Inherited (51)');
  assert.equal(fold.open, false);
  assert.ok([...list.querySelectorAll('.log-item')].every((item) => !item.classList.contains('is-inherited')));
  await settled();
  assert.equal(asked.length, 0, 'a folded fold fetches nothing');
});

test('opened, the fold lists the inherited Yard Logs 50 at a time, and the browser keeps it open', async () => {
  const asked = serving();
  const fold = foldOf(treeList(listing(), parse('#tree=%2Fwork%2Fshelf&branch=old-03')));
  fold.open = true;
  fold.dispatchEvent(new Event('toggle'));
  await settled();
  const items = () => [...fold.querySelectorAll('.inherited-logs .log-item')];
  assert.equal(items().length, 50);
  assert.equal(asked[0].get('repository'), '/work/shelf');
  assert.ok(items().every((item) => item.classList.contains('is-inherited')));
  assert.ok(items()[3].classList.contains('is-current'));
  const more = /** @type {HTMLButtonElement} */ (fold.querySelector('.more-button'));
  assert.equal(more.hidden, false);
  more.click();
  await settled();
  assert.equal(items().length, 51);
  assert.equal(more.hidden, true);
  assert.equal(prefs.getJson('inherited', {})['/work/shelf'], true);
  const again = foldOf(treeList(listing(), null));
  assert.equal(again.open, true);
  await settled();
  assert.equal(again.querySelectorAll('.inherited-logs .log-item').length, 51, 'a sidebar drawn again keeps those shown');
});

test('the filter lists only the inherited Yard Logs whose branch or title holds its text, whatever its case', async () => {
  const asked = serving();
  prefs.setJson('inherited', { '/work/shelf': true });
  const fold = foldOf(treeList(listing(), null));
  await settled();
  const filter = /** @type {HTMLInputElement} */ (fold.querySelector('.inherited-filter'));
  filter.value = 'READING';
  filter.dispatchEvent(new Event('input'));
  await settled(260);
  assert.equal(asked.at(-1)?.get('filter'), 'READING');
  assert.deepEqual([...fold.querySelectorAll('.inherited-logs .branch')].map((each) => each.textContent), ['old-07']);
});

test('a closed Yard Log is listed as closed, and an inherited one as inherited', async () => {
  serving();
  prefs.setJson('inherited', { '/work/shelf': true });
  const fold = foldOf(treeList(listing(), null));
  await settled();
  const first = fold.querySelector('.inherited-logs .log-item');
  assert.deepEqual([...(first?.querySelectorAll('.tag') ?? [])].map((tag) => tag.textContent), ['closed', 'inherited']);
});
