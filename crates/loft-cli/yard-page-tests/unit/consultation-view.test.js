// The consultation view: its charts by stage and by agent type, the share of the estimate, the
// partial and unmeasured notes, the labels a mark gives when pointed at or focused, the tables
// beneath the charts, and one hue for each class in both themes.

import { forget } from './dom.js';

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { beforeEach, test } from 'node:test';

import { consultationView } from '../../yard-page/views/consultation.js';
import { log, stage } from './fixtures.js';

beforeEach(forget);

const counts = (more = {}) => ({ specReads: 0, codeReads: 0, otherReads: 0, loftReads: 0, specEdits: 0, codeEdits: 0, ...more });

/** A Yard Log of two stages whose sessions were measured, `partial` or not, with `unmeasured` sessions. */
function shown({ partial = false, unmeasured = [] } = {}) {
  const stages = [stage('shape', 'Shaping', 'done', []), stage('build', 'Building', 'active', [])];
  const [shape, build] = stages.map((each) => each.key);
  const consultationSums = {
    measured: true,
    partial,
    all: { counts: counts({ specReads: 4, loftReads: 2, codeReads: 8, otherReads: 1, codeEdits: 3 }), estimate: { spec: 600, code: 300, other: 0, unclassified: 100 } },
    stages: [
      { stage: shape, title: 'Shaping', counts: counts({ specReads: 3, loftReads: 1, codeReads: 1 }), estimate: { spec: 0, code: 0, other: 0, unclassified: 0 }, codeEditsBeforeSpec: 0 },
      { stage: build, title: 'Building', counts: counts({ specReads: 1, loftReads: 1, codeReads: 7, otherReads: 1, codeEdits: 3 }), estimate: { spec: 0, code: 0, other: 0, unclassified: 0 }, codeEditsBeforeSpec: 2, specFirst: false },
    ],
    sessions: [],
    unmeasured,
    agents: [
      { type: 'main', counts: counts({ specReads: 4, loftReads: 2, codeReads: 2, codeEdits: 3 }), estimate: { spec: 0, code: 0, other: 0, unclassified: 0 } },
      { type: 'Explore', counts: counts({ codeReads: 6, otherReads: 1 }), estimate: { spec: 0, code: 0, other: 0, unclassified: 0 } },
    ],
  };
  return log(stages, { consultationSums });
}

/** The rows of the chart titled `title`: each row's name, its segments' classes and its total. */
function rows(drawn, title) {
  const chart = [...drawn.querySelectorAll('.consult-chart')].find((each) => each.querySelector('h4')?.textContent === title);
  assert.ok(chart, title);
  return [...chart.querySelectorAll('.consult-row')].map((row) => ({
    name: row.querySelector('.consult-name')?.textContent,
    marks: [...row.querySelectorAll('.consult-mark')].map((mark) => mark.getAttribute('class')?.replace('consult-mark ', '')),
    total: row.querySelector('.consult-total')?.textContent,
  }));
}

test('the view charts each stage’s reads in Yard Log’s order as one stacked bar, marked with which came first', () => {
  const drawn = consultationView(shown());
  assert.deepEqual(rows(drawn, 'By stage'), [
    { name: 'Shaping', marks: ['consult--specReads', 'consult--loftReads', 'consult--codeReads'], total: '5' },
    { name: 'Building', marks: ['consult--specReads', 'consult--loftReads', 'consult--codeReads', 'consult--otherReads'], total: '10✗ code first, 2 edits' },
  ]);
  const widths = [...drawn.querySelectorAll('.consult-chart')][0].querySelectorAll('.consult-row')[1].querySelectorAll('.consult-mark');
  assert.deepEqual([...widths].map((mark) => mark.getAttribute('width')), ['10%', '10%', '70%', '10%'], 'one scale for every bar of the chart');
});

test('the calls counted toward no stage are charted last, as no stage', () => {
  const log = shown();
  log.consultationSums.noStage = { counts: counts({ otherReads: 2 }), estimate: { spec: 0, code: 0, other: 0, unclassified: 0 }, codeEditsBeforeSpec: 0 };
  assert.deepEqual(rows(consultationView(log), 'By stage').map((row) => [row.name, row.total]), [
    ['Shaping', '5'],
    ['Building', '10✗ code first, 2 edits'],
    ['No stage', '2'],
  ]);
});

test('the view charts the same counts for the main agent and each type of subagent', () => {
  assert.deepEqual(rows(consultationView(shown()), 'By agent').map((row) => [row.name, row.total]), [
    ['main agent', '8'],
    ['Explore subagent', '7'],
  ]);
});

test('the view charts the share of the estimated tokens each class brought in, and names it an estimate', () => {
  const drawn = consultationView(shown());
  const estimate = drawn.querySelector('.consult-estimate');
  assert.match(estimate?.querySelector('h4')?.textContent ?? '', /an estimate$/);
  const marks = [...(estimate?.querySelectorAll('.consult-mark') ?? [])];
  assert.deepEqual(marks.map((mark) => mark.getAttribute('aria-label')), [
    'spec: an estimated 600 tokens (60%)',
    'code: an estimated 300 tokens (30%)',
    'unclassified: an estimated 100 tokens (10%)',
  ]);
  assert.equal(drawn.querySelector('.consult-partial'), null);
});

test('the view says when the estimate is partial, and which sessions are unmeasured', () => {
  const drawn = consultationView(shown({ partial: true, unmeasured: [{ session: 's2', host: 'codex' }] }));
  assert.match(drawn.querySelector('.consult-partial')?.textContent ?? '', /^The estimate is partial/);
  assert.equal(drawn.querySelector('.consult-unmeasured')?.textContent, 'Unmeasured: 1 session whose adapter does not count what its agents consult: s2 (codex)');
});

test('where nothing has been measured, the view says so and draws no chart', () => {
  const nothing = log([stage('shape', 'Shaping', 'active', [])], {
    consultationSums: { measured: false, partial: false, stages: [], sessions: [], unmeasured: [{ session: 's1', host: 'claude-code' }], agents: [] },
  });
  for (const drawn of [consultationView(nothing), consultationView(log([]))]) {
    assert.match(drawn.querySelector('.empty')?.textContent ?? '', /^Nothing has been measured yet/);
    assert.equal(drawn.querySelector('svg, .consult-chart'), null);
  }
});

test('each mark names its class, count and share when pointed at or focused, and can be focused', () => {
  const drawn = consultationView(shown());
  document.body.append(drawn);
  const mark = drawn.querySelectorAll('.consult-row')[1].querySelectorAll('.consult-mark')[2];
  const tip = /** @type {HTMLElement} */ (drawn.querySelector('.consult-tip'));
  assert.equal(mark.getAttribute('tabindex'), '0');
  assert.equal(mark.getAttribute('aria-label'), 'code reads: 7 of 10 (70%)');
  assert.ok(tip.hidden);
  mark.dispatchEvent(new Event('focusin', { bubbles: true }));
  assert.equal(tip.hidden, false);
  assert.equal(tip.textContent, 'code reads: 7 of 10 (70%)');
  mark.dispatchEvent(new Event('focusout', { bubbles: true }));
  assert.ok(tip.hidden);
  mark.dispatchEvent(new Event('pointerover', { bubbles: true }));
  assert.equal(tip.textContent, 'code reads: 7 of 10 (70%)');
});

test('each chart has a table of its counts beneath it', () => {
  const drawn = consultationView(shown());
  const tables = [...drawn.querySelectorAll('.consult-chart')].map((chart) => chart.querySelector('details.consult-table table'));
  assert.equal(tables.length, 3);
  assert.ok(tables.every(Boolean));
  const building = [...(tables[0]?.querySelectorAll('tbody tr') ?? [])][1];
  assert.deepEqual([...(building?.children ?? [])].map((cell) => cell.textContent), ['Building', '1', '1', '7', '1', '0', '3', 'code first, 2 edits']);
});

test('each class is drawn in one hue in every chart, in the light and the dark theme alike', () => {
  const css = readFileSync(new URL('../../yard-page/yard.css', import.meta.url), 'utf8');
  const dark = css.slice(css.indexOf('@media (prefers-color-scheme: dark)'));
  for (const hue of ['--consult-spec', '--consult-loft', '--consult-code', '--consult-other', '--consult-none']) {
    assert.match(css.slice(0, css.indexOf('@media (prefers-color-scheme: dark)')), new RegExp(`${hue}: #[0-9a-f]{6};`), `${hue} in the light theme`);
    assert.match(dark, new RegExp(`${hue}: #[0-9a-f]{6};`), `${hue} in the dark theme`);
  }
  assert.match(css, /\.consult--specReads, \.consult--spec \{ color: var\(--consult-spec\); fill: var\(--consult-spec\); \}/);
  assert.match(css, /\.consult--codeReads, \.consult--code \{ color: var\(--consult-code\); fill: var\(--consult-code\); \}/);
});
