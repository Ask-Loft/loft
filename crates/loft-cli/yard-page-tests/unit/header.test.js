// The page's head: where the Yard Log lies, its title and the line that sums it up, and the
// controls of the view shown at its right, the order of the stages and the button that opens what
// waits on the person; the tabs of the views along its foot; Loft's wordmark, its L in action
// blue; and the view chosen, kept in the browser.

import { forget, root } from './dom.js';

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { beforeEach, test } from 'node:test';

import { add } from '../../yard-page/lib/dom.js';
import * as prefs from '../../yard-page/lib/prefs.js';
import { logHead, logView } from '../../yard-page/views/log.js';
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
  const head = at.querySelector('#head');
  if (head) add(head, logHead(shown, context));
  at.append(logView(shown, context));
  return at;
}

test('each view chosen in the head’s tabs is kept in the browser and drawn alone', () => {
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
  assert.ok(!tokens.querySelector('.timeline, .gantt, .consultation'));
  prefs.set('view', 'consultation');
  const consulted = page(shown);
  assert.ok(consulted.querySelector('.yard-log.view--consultation .consultation'));
  assert.ok(!consulted.querySelector('.timeline, .gantt, .tokens'));
});

test('the button that opens what waits lies in the page’s head', () => {
  const asked = question('colour', ['Red', 'Blue']);
  const shown = log([stage('s', 'S', 'active', [asked])], { submissions: [open([asked], '2026-01-01T10:04:00Z')] });
  const drawn = page(shown);
  const button = drawn.querySelector('header#head #open-waiting');
  assert.ok(button, 'the head holds the button');
  assert.equal(drawn.querySelectorAll('.toolbar #open-waiting').length, 0);
});

test('the switch of order lies among the controls of the page’s head, shown only with the timeline', () => {
  const shown = log([stage('s', 'S', 'active', [])]);
  let drawnAgain = 0;
  const drawn = page(shown, { onView: () => (drawnAgain += 1) });
  const select = /** @type {HTMLSelectElement} */ (drawn.querySelector('#head .head-controls select#order'));
  assert.ok(select, 'the head’s controls hold the order');
  assert.deepEqual([...select.options].map((option) => [option.value, option.textContent]), [
    ['newest', 'Newest first'],
    ['oldest', 'Oldest first'],
  ]);
  select.value = 'oldest';
  select.dispatchEvent(new Event('change'));
  assert.equal(prefs.order(), 'oldest');
  assert.equal(drawnAgain, 1);
  assert.equal(drawn.querySelectorAll('.yard-log select, .toolbar').length, 0, 'nothing of the order lies beneath the head');
  prefs.set('view', 'gantt');
  assert.equal(page(shown).querySelectorAll('#order').length, 0);
  prefs.set('order', null);
});

test('each control of the head is labelled above it in small text', () => {
  const asked = question('colour', ['Red', 'Blue']);
  const shown = log([stage('s', 'S', 'active', [asked])], { submissions: [open([asked], '2026-01-01T10:04:00Z')] });
  const controls = [...page(shown).querySelectorAll('#head .head-controls > .control')];
  assert.deepEqual(controls.map((control) => control.firstElementChild?.className), ['control-label', 'control-label']);
  assert.deepEqual(controls.map((control) => control.firstElementChild?.textContent), ['Order of the stages', 'Asked of you']);
  assert.ok(/\.control-label\s*\{[^}]*font-size:\s*12px/.test(css), 'the label is small');
  assert.ok(/\.control\s*\{[^}]*display:\s*grid/.test(css), 'the label stands above its control');
});

test('the head names the repository, the worktree where it is not the main one, and the branch, above the title', () => {
  const shown = log([stage('s', 'S', 'active', [])]);
  const main = page(shown, { place: { repository: '/work/shelf', main: true } });
  assert.deepEqual([...main.querySelectorAll('#head .kicker > span')].map((span) => span.textContent), ['shelf', 'feat/shelf']);
  forget();
  const other = page(shown, { tree: '/work/shelf-docs', place: { repository: '/work/shelf', main: false } });
  assert.deepEqual([...other.querySelectorAll('#head .kicker > span')].map((span) => span.textContent), ['shelf', 'shelf-docs', 'feat/shelf']);
  const kicker = other.querySelector('#head .kicker');
  assert.ok(kicker?.compareDocumentPosition(other.querySelector('#head h1#log-title')) & Node.DOCUMENT_POSITION_FOLLOWING);
});

test('the head’s title is Yard Log’s title, and the line beneath it counts the stages done, active and planned', () => {
  const shown = log([stage('a', 'A', 'done', []), stage('b', 'B', 'done', []), stage('c', 'C', 'active', []), stage('d', 'D', 'planned', [])]);
  const drawn = page(shown);
  assert.equal(drawn.querySelector('#head h1#log-title')?.textContent, shown.yardLog.title);
  assert.equal(drawn.querySelector('#head .summary')?.textContent, '4 stages: 2 done, 1 active, 1 planned');
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

test('beneath the line that sums up the stages, the head holds the tabs Timeline, Gantt chart, Tokens and Consultation, the one shown current', () => {
  const shown = log([stage('s', 'S', 'active', [])]);
  prefs.set('view', 'gantt');
  const head = page(shown).querySelector('#head');
  const tabs = [...(head?.querySelectorAll('.view-tabs .view-tab') ?? [])];
  assert.deepEqual(tabs.map((tab) => tab.textContent), ['Timeline', 'Gantt chart', 'Tokens', 'Consultation']);
  assert.deepEqual(tabs.map((tab) => tab.getAttribute('aria-current')), [null, 'page', null, null]);
  assert.ok(tabs.every((tab) => tab.querySelector('svg')), 'each tab has its icon');
  const summary = head?.querySelector('.summary');
  assert.ok(summary && summary.compareDocumentPosition(tabs[0]) & Node.DOCUMENT_POSITION_FOLLOWING, 'the tabs follow the summary');
});

test('choosing a view’s tab shows that view and keeps the stage chosen', () => {
  const shown = log([stage('s', 'S', 'active', [])]);
  let drawn = 0;
  const head = page(shown, { chosen: { stage: 's' }, onView: () => (drawn += 1) }).querySelector('#head');
  /** @type {HTMLElement | null | undefined} */ (head?.querySelector('[data-view="tokens"]'))?.click();
  assert.equal(prefs.view(), 'tokens');
  assert.equal(drawn, 1, 'the page is drawn again');
  /** @type {HTMLElement | null | undefined} */ (head?.querySelector('[data-view="timeline"]'))?.click();
  assert.equal(drawn, 2);
  assert.ok(page(shown, { chosen: { stage: 's' } }).querySelector('.stage-pane #block-s'), 'the stage chosen stays chosen');
});

test('the tab of the view shown is ink, underlined in action blue', () => {
  const rule = css.match(/\.view-tab\[aria-current="page"\]\s*\{([^}]*)\}/)?.[1] ?? '';
  assert.match(rule, /color:\s*var\(--ink\)/);
  assert.match(rule, /border-bottom-color:\s*var\(--signal\)/);
});

test('the head names the branch above the title only where the title is not the branch’s name', () => {
  const shown = log([stage('s', 'S', 'active', [])]);
  shown.yardLog.title = shown.branch ?? 'feat/shelf';
  const drawn = page(shown, { place: { repository: '/work/shelf', main: true } });
  assert.equal(drawn.querySelector('#head h1#log-title')?.textContent, shown.yardLog.title);
  assert.deepEqual([...drawn.querySelectorAll('#head .kicker > span')].map((span) => span.textContent), ['shelf']);
});
