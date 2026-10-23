// The page's header: the Yard mark, its Y in the website's action blue, then the switch of views
// and the button that opens what waits on the person; and the line above the stages that keeps
// the switch of order.

import { forget, root } from './dom.js';

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { beforeEach, test } from 'node:test';

import { add } from '../../yard-page/lib/dom.js';
import * as prefs from '../../yard-page/lib/prefs.js';
import { logChrome, logView } from '../../yard-page/views/log.js';
import { log, open, question, stage } from './fixtures.js';

beforeEach(() => {
  forget();
  // The view chosen is kept in memory too, where storage refuses it; each test begins with none.
  prefs.set('view', null);
});

const html = readFileSync(new URL('../../yard-page/index.html', import.meta.url), 'utf8');
const css = readFileSync(new URL('../../yard-page/yard.css', import.meta.url), 'utf8');

/** The header of the page as `index.html` writes it, and a Yard Log's page drawn under it. */
function page(shown, more = {}) {
  const at = root();
  at.innerHTML = html.match(/<header class="top">[\s\S]*?<\/header>/)?.[0] ?? '';
  const context = {
    tree: '/work/shelf',
    branch: 'feat/shelf',
    last: null,
    now: Date.parse('2026-01-01T12:00:00Z'),
    width: 960,
    refresh: () => {},
    notify: () => {},
    onView: () => {},
    onPick: () => {},
    onWaiting: () => {},
    ...more,
  };
  const chrome = at.querySelector('#chrome');
  if (chrome) add(chrome, logChrome(shown, context));
  at.append(logView(shown, context));
  return at;
}

test('the switch of views lies in the page’s header after the Yard mark, the view shown pressed', () => {
  const drawn = page(log([stage('s', 'S', 'active', [])]));
  const header = drawn.querySelector('header.top');
  const views = header.querySelector('nav.views');
  assert.ok(views, 'the header holds the switch of views');
  assert.ok(header.querySelector('.mark').compareDocumentPosition(views) & Node.DOCUMENT_POSITION_FOLLOWING);
  const tabs = [...views.querySelectorAll('button.view-tab')].map((tab) => [tab.textContent, tab.getAttribute('aria-pressed')]);
  assert.deepEqual(tabs, [
    ['Timeline', 'true'],
    ['Gantt chart', 'false'],
  ]);
  assert.equal(drawn.querySelectorAll('.toolbar .view-tab, .toolbar [aria-label="View"]').length, 0);
});

test('the chosen view is pressed in the header, and choosing the other is kept in the browser', () => {
  let drawnAgain = 0;
  const drawn = page(log([stage('s', 'S', 'active', [])]), { onView: () => (drawnAgain += 1) });
  /** @type {any} */ (drawn.querySelector('nav.views button.view-tab:nth-child(2)')).click();
  assert.equal(prefs.view(), 'gantt');
  assert.equal(drawnAgain, 1);
  const again = page(log([stage('s', 'S', 'active', [])]));
  assert.equal(again.querySelector('nav.views [aria-pressed="true"]')?.textContent, 'Gantt chart');
});

test('the button that opens what waits follows the switch of views in the page’s header', () => {
  const asked = question('colour', ['Red', 'Blue']);
  const shown = log([stage('s', 'S', 'active', [asked])], { submissions: [open([asked], '2026-01-01T10:04:00Z')] });
  const drawn = page(shown);
  const button = drawn.querySelector('header.top #open-waiting');
  assert.ok(button, 'the header holds the button');
  assert.ok(drawn.querySelector('nav.views').compareDocumentPosition(button) & Node.DOCUMENT_POSITION_FOLLOWING);
  assert.equal(drawn.querySelectorAll('.toolbar #open-waiting').length, 0);
});

test('the switch of order lies at the left of the line above the stages, shown only with the timeline', () => {
  const shown = log([stage('s', 'S', 'active', [])]);
  const drawn = page(shown);
  const first = drawn.querySelector('.toolbar')?.firstElementChild;
  assert.equal(first?.getAttribute('aria-label'), 'Order of the stages');
  assert.equal(drawn.querySelectorAll('header.top [aria-label="Order of the stages"]').length, 0);
  prefs.set('view', 'gantt');
  assert.equal(page(shown).querySelectorAll('[aria-label="Order of the stages"]').length, 0);
});

test('the Y of the Yard mark is action blue and the rest of the name ink, in either scheme', () => {
  const drawn = root();
  drawn.innerHTML = html;
  const word = drawn.querySelector('.mark-word');
  assert.equal(word?.textContent, 'Yard');
  assert.equal(word?.querySelector('.mark-initial')?.textContent, 'Y');
  assert.ok(/\.mark-initial\s*\{[^}]*color:\s*var\(--signal\)/.test(css), 'the Y is drawn in --signal');
  assert.ok(!/\.mark-word\s*\{[^}]*color:/.test(css), 'the rest of the name takes the page’s ink');
  // The website's L: #14488f in the light scheme, #8ec5f2 in the dark one.
  assert.match(css.match(/^:root\s*\{([^}]*)\}/m)?.[1] ?? '', /--signal:\s*#14488f;/);
  assert.match(css.match(/@media \(prefers-color-scheme: dark\)\s*\{\s*:root\s*\{([^}]*)\}/)?.[1] ?? '', /--signal:\s*#8ec5f2;/);
});
