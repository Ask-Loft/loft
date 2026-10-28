// What the page keeps of each Yard Log in this browser: its folds and its Gantt chart's scroll.

import { forget, root } from './dom.js';

import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';

import * as kept from '../../yard-page/lib/kept.js';
import * as prefs from '../../yard-page/lib/prefs.js';
import { blocksByKey, decisionsByQuestion } from '../../yard-page/model/log.js';
import { loftView } from '../../yard-page/views/loft/index.js';
import { timeline } from '../../yard-page/views/timeline.js';
import { block, log as yardLog, stage } from './fixtures.js';

beforeEach(() => {
  forget();
  // What the page holds in memory beside storage goes with it, as a page loaded afresh holds none.
  prefs.set('kept', null);
  kept.reset();
});

/** A Yard Log keyed `key`, holding a stage `stage` with the blocks `blocks`. */
function log(key, blocks = ['note-1'], stage = 'stage-1') {
  return { yardLog: { key }, stages: [{ key: stage, blocks: blocks.map((each) => ({ key: each })) }] };
}

/** What the browser holds, once the page has written it. */
function stored() {
  kept.flush();
  return JSON.parse(localStorage.getItem('yard.kept') ?? '{}');
}

/** A fold drawn through the store, toggled as the person would. */
function toggle(details) {
  details.open = !details.open;
  details.dispatchEvent(new Event('toggle'));
}

test('a connected fold is saved before its deferred native toggle event arrives', () => {
  kept.begin(log('log-a'));
  const fold = kept.details({ path: 'log-a/tokens', open: false }, {});
  root().append(fold);
  fold.open = true;
  assert.equal(stored()['log-a'].folds['log-a/tokens']?.value, true);
  kept.reset();
  kept.begin(log('log-a'));
  assert.equal(kept.details({ path: 'log-a/tokens', open: false }, {}).open, true);
});

test('a redraw captures the current fold state and ignores a replaced elements late toggle', () => {
  kept.begin(log('log-a'));
  const fold = kept.details({ path: 'log-a/tokens', open: false }, {});
  root().append(fold);
  toggle(fold);
  fold.open = false;
  kept.begin(log('log-a'));
  const next = kept.details({ path: 'log-a/tokens', open: false }, {});
  root().replaceChildren(next);
  assert.equal(next.open, false);
  fold.open = true;
  fold.dispatchEvent(new Event('toggle'));
  assert.equal(kept.get('log-a/tokens', false), false);
});

test('a fold the person toggled is drawn so again, by a fresh draw and after the page loads again', () => {
  kept.begin(log('log-a'));
  const first = kept.details({ path: 'note-1/note', open: false }, { class: 'fold' }, 'text');
  assert.equal(first.open, false);
  toggle(first);
  assert.equal(kept.details({ path: 'note-1/note', open: false }, {}).open, true);
  kept.flush();
  kept.reset();
  kept.begin(log('log-a'));
  assert.equal(kept.details({ path: 'note-1/note', open: false }, {}).open, true);
});

test('a fold is kept only while it differs from its default', () => {
  kept.begin(log('log-a'));
  const fold = kept.details({ path: 'note-1/note', open: false }, {});
  toggle(fold);
  assert.deepEqual(Object.keys(stored()['log-a'].folds), ['note-1/note']);
  toggle(fold);
  assert.deepEqual(stored()['log-a'].folds, {});
});

test('a stage the person folded takes its default again once its status changes', () => {
  kept.begin(log('log-a'));
  const stage = { path: 'stage-1/open', open: true, basis: 'active' };
  toggle(kept.details(stage, {}));
  assert.equal(kept.details(stage, {}).open, false);
  assert.equal(kept.details({ path: 'stage-1/open', open: false, basis: 'done' }, {}).open, false);
  assert.equal(kept.details({ path: 'stage-1/open', open: true, basis: 'active' }, {}).open, false);
  kept.set('stage-1/open', true, false, 'done');
  assert.equal(kept.get('stage-1/open', false, 'done'), true);
});

test('the browser forgets the folds of blocks a Yard Log no longer holds', () => {
  kept.begin(log('log-a', ['note-1', 'note-2']));
  kept.set('note-1/note', true, false);
  kept.set('note-2/note', true, false);
  kept.set('stage-1/open', false, true, 'active');
  kept.begin(log('log-a', ['note-2']));
  assert.deepEqual(Object.keys(stored()['log-a'].folds).sort(), ['note-2/note', 'stage-1/open']);
});

test('the browser keeps the 30 Yard Logs opened last and forgets the others', () => {
  for (let index = 0; index < 30; index += 1) {
    kept.begin(log(`log-${index}`));
    kept.set('note-1/note', true, false);
  }
  kept.begin(log('log-0'));
  kept.begin(log('log-30'));
  const logs = Object.keys(stored());
  assert.equal(logs.length, 30);
  assert.ok(!logs.includes('log-1'));
  kept.begin(log('log-0'));
  assert.equal(kept.get('note-1/note', false), true);
});

test('each Yard Log keeps its own folds and its own scroll of the Gantt chart', () => {
  kept.begin(log('log-a'));
  kept.set('note-1/note', true, false);
  kept.setScroll({ left: 120, top: 40 });
  kept.begin(log('log-b'));
  assert.equal(kept.get('note-1/note', false), false);
  assert.equal(kept.scroll(), null);
  kept.begin(log('log-a'));
  assert.deepEqual(kept.scroll(), { left: 120, top: 40 });
  assert.equal(kept.get('note-1/note', false), true);
});

test('a write from another tab to another Yard Log is not lost when this page writes', () => {
  kept.begin(log('log-a'));
  kept.set('note-1/note', true, false);
  kept.flush();
  const other = JSON.parse(localStorage.getItem('yard.kept') ?? '{}');
  other['log-b'] = { at: 99, folds: { 'note-1/note': { value: true } } };
  localStorage.setItem('yard.kept', JSON.stringify(other));
  kept.setScroll({ left: 1, top: 2 });
  assert.deepEqual(Object.keys(stored()).sort(), ['log-a', 'log-b']);
});

/** The page loaded again: what was written is all that is left. */
function reload(shown) {
  kept.flush();
  kept.reset();
  kept.begin(shown);
}

/** The timeline of `shown`, oldest first. */
function drawTimeline(shown) {
  return timeline(shown, {
    blocks: blocksByKey(shown),
    decisions: decisionsByQuestion(shown),
    marked: new Set(),
    waiting: new Set(),
    order: 'oldest',
    modes: [],
    now: Date.parse('2026-01-01T12:00:00Z'),
  });
}

/** `shown` with its first stage's status `status`. */
function moved(shown, status) {
  const [first] = shown.stages;
  return { ...shown, stages: [{ ...first, version: { ...first.version, body: { ...first.version.body, status } } }] };
}

test('a stage and a folded note the person toggled stay so through a fresh draw and after the page loads again', () => {
  const note = block('note', 'kept-note', { title: 'Folded', folded: true, html: '<p>Held.</p>' });
  const shown = yardLog([stage('now', 'Now', 'active', [note])]);
  kept.begin(shown);
  const first = drawTimeline(shown);
  toggle(first.querySelector('details.stage'));
  toggle(first.querySelector('#block-kept-note details'));
  for (const drawn of [drawTimeline(shown), (reload(shown), drawTimeline(shown))]) {
    assert.equal(drawn.querySelector('details.stage').open, false);
    assert.equal(drawn.querySelector('#block-kept-note details').open, true);
  }
});

test('a planned stage the person opened is folded again once it is done', () => {
  const shown = yardLog([stage('later', 'Later', 'planned', [])]);
  kept.begin(shown);
  toggle(drawTimeline(shown).querySelector('details.stage'));
  assert.equal(drawTimeline(shown).querySelector('details.stage').open, true);
  const done = moved(shown, 'done');
  kept.begin(done);
  assert.equal(drawTimeline(done).querySelector('details.stage').open, false);
});

test('the Feature Group the person opened in a spec-change view is open again after the page loads again', () => {
  const nodes = [{ key: 'p', kind: 'product', label: 'P-1', name: 'Reading List', parent: null, change: null }];
  for (let g = 0; g < 4; g += 1) {
    nodes.push({ key: `g${g}`, kind: 'feature_group', label: `FG-${g}`, name: `Group ${g}`, parent: 'p', change: null });
    for (let f = 0; f < 9; f += 1) nodes.push({ key: `g${g}f${f}`, kind: 'feature', label: null, name: `Feature ${f}`, parent: `g${g}`, change: 'added' });
  }
  const body = {
    view: 'spec-change',
    recorded: { commit: 'abcdef0123456789', branch: 'feat/shelf', loft: '2.2.1', at: '2026-01-01T10:00:00Z' },
    report: { status: { base: { revision: 'develop', added: [{ kind: 'feature', name: 'Feature 0' }], changed: [], removed: [] } }, graph: { nodes, decisions: [], edges: [] } },
  };
  const view = block('loft', 'kept-view', body);
  const shown = yardLog([stage('now', 'Now', 'active', [view])]);
  const pressed = (drawn) => [...drawn.querySelectorAll('.group-button')].filter((button) => button.getAttribute('aria-pressed') === 'true').map((button) => button.textContent);
  kept.begin(shown);
  const first = loftView(body, { key: view.key });
  /** @type {HTMLButtonElement} */ ([...first.querySelectorAll('.group-button')][1]).click();
  assert.deepEqual(pressed(first), ['FG-1 Group 1 (9)']);
  reload(shown);
  assert.deepEqual(pressed(loftView(body, { key: view.key })), ['FG-1 Group 1 (9)']);
});
