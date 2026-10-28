// Delegated work stays in its original stage and paints only recorded execution attempts.
import { forget } from './dom.js';
import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';
import { chart } from '../../yard-page/model/gantt.js';
import { gantt } from '../../yard-page/views/gantt.js';
import { card } from '../../yard-page/views/blocks.js';
import { blocksByKey, decisionsByQuestion } from '../../yard-page/model/log.js';
import { MAIN, block, log, stage } from './fixtures.js';

beforeEach(forget);
const T0 = Date.parse('2026-01-01T10:00:00Z');
const at = (n) => new Date(T0 + n * 1000).toISOString();
const worker = (id) => ({ host: 'codex', session: 's1', agent: id, agentType: 'builder' });
function assigned(id, attempts, state = 'active') {
  return block('task', id, { title: id, state, assignee: worker(id) }, { more: {
    times: { listed: at(0), started: at(5) },
    executions: attempts.map(([from, to]) => ({ started: at(from), ...(to === undefined ? {} : { ended: at(to) }), assignee: worker(id) })),
  } });
}
function shown(tasks) {
  return log([
    stage('build', 'Build', 'done', tasks, { owner: MAIN, times: { started: at(0), ended: at(10) } }),
    stage('next', 'Next', 'active', [], { owner: MAIN, times: { started: at(10) } }),
  ]);
}

test('concurrent assignments keep separate bars beneath the original stage after the owner moves on', () => {
  const view = shown([assigned('a', [[5, 15]]), assigned('b', [[10, 20]])]);
  const model = chart(view, T0 + 30_000);
  assert.equal(model.lanes.length, 1);
  assert.deepEqual(model.lanes[0].rows.map(row => row.tasks.map(task => task.id)), [['a', 'b'], []]);
  const drawn = gantt(view, { width: 900, now: T0 + 30_000, onPick: () => {} });
  assert.equal(drawn.querySelectorAll('.task-row').length, 2);
  assert.equal(drawn.querySelectorAll('.task-active').length, 2);
  assert.match(drawn.querySelector('.task-row').textContent, /Assigned to.*codex.*s1.*a.*builder/);
});

test('overlapping assignments count elapsed execution once in the total work', () => {
  const model = chart(shown([assigned('a', [[5, 15]]), assigned('b', [[10, 20]])]), T0 + 30_000);
  assert.equal(model.summary.total, 30_000);
  assert.equal(model.summary.parts.find(part => part.kind === 'task').length, 15_000);
  assert.equal(model.summary.parts.reduce((sum, part) => sum + part.length, 0), model.summary.total);
});

test('returning to open ends active paint and a later execution leaves the pause visible', () => {
  const paused = assigned('a', [[5, 10]], 'open');
  const resumed = assigned('b', [[6, 9], [20, undefined]]);
  const view = shown([paused, resumed]);
  const tasks = chart(view, T0 + 30_000).lanes[0].rows[0].tasks;
  const spans = tasks.map(task => task.active.map(part => [(part.from - T0) / 1000, (part.to - T0) / 1000]));
  assert.deepEqual(spans, [[[5, 10]], [[6, 9], [20, 30]]]);
  const drawn = gantt(view, { width: 900, now: T0 + 30_000, onPick: () => {} });
  assert.equal(drawn.querySelectorAll('.task-active').length, 3);
  const first = drawn.querySelector('.task-row .task-active');
  assert.match(first.title, /codex.*s1.*a/);
});

test('a task card distinguishes the assignee from the stage owner and version author', () => {
  const task = assigned('a', [[5, 10]], 'open');
  const view = shown([task]);
  const context = { blocks: blocksByKey(view), decisions: decisionsByQuestion(view), marked: new Set(), waiting: new Set() };
  const drawn = card(task, context);
  const assignment = drawn.querySelector('.task-assignee');
  assert.match(assignment.textContent, /Assigned to.*codex.*s1.*a.*builder/);
  assert.match(drawn.querySelector('.block-meta').textContent, /main agent/);
});
