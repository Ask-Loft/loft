// The page's head: the button that opens what waits on the person, and the line above the stages
// that keeps the switch of order; Loft's wordmark, its L in action blue; and the view chosen in
// the sidebar, kept in the browser.

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
  at.innerHTML = html.match(/<header id="head"[\s\S]*?<\/header>/)?.[0] ?? '';
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

test('each view chosen in the sidebar is kept in the browser and drawn alone', () => {
  const shown = log([stage('s', 'S', 'active', [])]);
  assert.equal(prefs.view(), 'timeline');
  assert.ok(page(shown).querySelector('.timeline'));
  assert.ok(!page(shown).querySelector('.tokens'), 'the timeline draws no tokens beneath it');
  prefs.set('view', 'gantt');
  assert.equal(localStorage.getItem('yard.view'), 'gantt');
  const gantt = page(shown);
  assert.ok(gantt.querySelector('.yard-log.view--gantt'));
  assert.ok(!gantt.querySelector('.tokens, .timeline'), 'the Gantt chart draws no tokens beneath it');
  prefs.set('view', 'tokens');
  const tokens = page(shown);
  assert.ok(tokens.querySelector('.tokens'));
  assert.ok(!tokens.querySelector('.timeline, .gantt'));
});

test('the button that opens what waits lies in the page’s head', () => {
  const asked = question('colour', ['Red', 'Blue']);
  const shown = log([stage('s', 'S', 'active', [asked])], { submissions: [open([asked], '2026-01-01T10:04:00Z')] });
  const drawn = page(shown);
  const button = drawn.querySelector('header#head #open-waiting');
  assert.ok(button, 'the head holds the button');
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

test('the L of Loft’s wordmark is action blue and the rest of the name ink, in either scheme', () => {
  assert.ok(/\.wordmark-initial\s*\{[^}]*color:\s*var\(--signal\)/.test(css), 'the L is drawn in --signal');
  assert.ok(!/\.wordmark\s*\{[^}]*color:/.test(css), 'the rest of the name takes the sidebar’s ink');
  assert.ok(/\.app-name\s*\{[^}]*color:\s*var\(--muted\)/.test(css), 'Yard beside it is muted');
  assert.ok(/\.wordmark-link\s*\{[^}]*align-items:\s*baseline/.test(css), 'the wordmark and Yard share a baseline');
  // Loft Web's action blue: #14488f in the light scheme, #8ec5f2 in the dark one.
  assert.match(css.match(/^:root\s*\{([^}]*)\}/m)?.[1] ?? '', /--signal:\s*#14488f;/);
  assert.match(css.match(/@media \(prefers-color-scheme: dark\)\s*\{\s*:root\s*\{([^}]*)\}/)?.[1] ?? '', /--signal:\s*#8ec5f2;/);
});

test('the page names no other mark than Loft’s wordmark and Yard', () => {
  const drawn = root();
  drawn.innerHTML = html.match(/<body>[\s\S]*<\/body>/)?.[0] ?? '';
  assert.equal(drawn.querySelectorAll('.mark, .mark-word, .dots').length, 0);
  assert.ok(!/\.(mark-word|dots)\b/.test(css), 'the stylesheet draws no other mark');
});
