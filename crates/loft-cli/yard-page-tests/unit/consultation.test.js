// What the agents consulted: the line beside each stage, in the timeline and the Gantt chart.

import { forget } from './dom.js';

import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';

import { blocksByKey, decisionsByQuestion } from '../../yard-page/model/log.js';
import { line, summary } from '../../yard-page/model/consultation.js';
import { gantt } from '../../yard-page/views/gantt.js';
import { timeline } from '../../yard-page/views/timeline.js';
import { MAIN, log, stage } from './fixtures.js';

beforeEach(forget);

const T0 = Date.parse('2026-01-01T10:00:00Z');
const at = (seconds) => new Date(T0 + seconds * 1000).toISOString().replace('.000', '');

/** Counts of reads and edits, every one not given none. */
const counts = (more = {}) => ({ specReads: 0, codeReads: 0, otherReads: 0, loftReads: 0, specEdits: 0, codeEdits: 0, ...more });
const estimate = { spec: 0, code: 0, other: 0, unclassified: 0 };

/** A stage's sums: `more` counts, with the order its edits of code give. */
function scope(stage, more, order = {}) {
  return { stage, counts: counts(more), estimate, codeEditsBeforeSpec: 0, ...order };
}

/** A Yard Log of three stages, `led` led by the specification, `rushed` not, `quiet` with nothing consulted. */
function shown() {
  const stages = [
    stage('led', 'Led', 'done', [], { owner: MAIN, times: { started: at(0), ended: at(100) } }),
    stage('rushed', 'Rushed', 'done', [], { owner: MAIN, times: { started: at(100), ended: at(200) } }),
    stage('quiet', 'Quiet', 'active', [], { owner: MAIN, times: { started: at(200) } }),
  ];
  const [led, rushed, quiet] = stages.map((each) => each.key);
  const consultationSums = {
    measured: true,
    partial: false,
    stages: [
      scope(led, { specReads: 3, loftReads: 1, codeReads: 9, codeEdits: 2 }, { specFirst: true }),
      scope(rushed, { specReads: 1, codeReads: 4, codeEdits: 3 }, { specFirst: false, codeEditsBeforeSpec: 2 }),
      scope(quiet, {}),
    ],
    sessions: [],
    unmeasured: [],
    agents: [],
  };
  return log(stages, { consultationSums, activity: [{ host: 'claude-code', session: 's1', turns: [{ started: at(0), ended: at(300) }], plan: [], spec: [], loft: [] }] });
}

test('the line gives the consultations of the specification, the reads of code and which came first', () => {
  const [led, rushed, quiet] = shown().consultationSums.stages;
  assert.equal(line(led)?.text, 'spec 4 · code 9 · spec first');
  assert.equal(line(rushed)?.text, 'spec 1 · code 4 · code first, 2 edits');
  assert.equal(line(rushed)?.said, '1 consultation of the specification; 4 reads of code; 2 edits of code came before the specification was consulted');
  assert.equal(line(quiet), null);
  assert.equal(line(undefined), null);
  assert.equal(line(scope('read', { codeReads: 2 }))?.text, 'spec 0 · code 2');
});

test('the page keys the page server’s sums by stage and sums nothing itself', () => {
  const sums = shown().consultationSums;
  const kept = summary(sums);
  assert.equal(kept.byStage.get(sums.stages[1].stage), sums.stages[1]);
  assert.equal(summary(undefined).measured, false);
});

/** The line drawn in each of `places`, by its stage's id. */
function lines(drawn, selector) {
  return [...drawn.querySelectorAll(selector)].map((each) => each.textContent);
}

test('the timeline draws the line in each stage’s row and in the chosen stage’s head, and none for a stage that consulted nothing', () => {
  const log = shown();
  const context = {
    blocks: blocksByKey(log),
    decisions: decisionsByQuestion(log),
    marked: new Set(),
    waiting: new Set(),
    order: 'oldest',
    modes: [],
    now: T0 + 400_000,
    chosen: { stage: 'rushed' },
  };
  const drawn = timeline(log, context);
  assert.deepEqual(
    ['led', 'rushed', 'quiet'].map((id) => drawn.querySelector(`#row-${id} .consult-line`)?.textContent ?? null),
    ['spec 4 · code 9 · spec first', 'spec 1 · code 4 · code first, 2 edits', null],
  );
  assert.ok(drawn.querySelector('#row-led .consult-line--spec-first'));
  assert.ok(drawn.querySelector('#row-rushed .consult-line--code-first'));
  assert.deepEqual(lines(drawn, '.stage-head .consult-line'), ['spec 1 · code 4 · code first, 2 edits']);
});

test('the Gantt chart draws the line in brief beside each stage’s name, its words for a reader', () => {
  const drawn = gantt(shown(), { width: 900, now: T0 + 400_000, onPick: () => {} });
  const brief = [...drawn.querySelectorAll('.stage-row .name-cell')].map((cell) => cell.querySelector('.consult-line'));
  assert.deepEqual(
    brief.map((each) => each?.querySelector('[aria-hidden]')?.textContent ?? null),
    ['4·9 ✓', '1·4 ✗2', null],
  );
  assert.equal(brief[1]?.querySelector('.sr-only')?.textContent, 'Consulted: 1 consultation of the specification; 4 reads of code; 2 edits of code came before the specification was consulted');
});
