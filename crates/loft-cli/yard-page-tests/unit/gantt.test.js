// Yard Log along time: the Gantt chart's model and its drawing.

import { forget } from './dom.js';

import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';

import { FOLD_AFTER, chart, scale } from '../../yard-page/model/gantt.js';
import { gantt } from '../../yard-page/views/gantt.js';
import { EXPLORE, MAIN, act, answer, block, log, stage } from './fixtures.js';

beforeEach(forget);

const T0 = Date.parse('2026-01-01T10:00:00Z');
/** The time `seconds` after T0, as the page server writes it. */
const at = (seconds) => new Date(T0 + seconds * 1000).toISOString().replace('.000', '');
const span = (from, to) => ({ started: at(from), ended: to === undefined ? undefined : at(to) });

/** A stage owned by `owner` with the times given in seconds after T0. */
function timed(id, owner, from, to, blocks = [], status = 'done') {
  return stage(id, id, status, blocks, { owner, times: span(from, to) });
}

/** A session's activity: its turns, plan and spec stretches and Loft runs. */
function activity(more = {}) {
  return { session: 's1', turns: [span(0, 600)], plan: [], spec: [], loft: [], ...more };
}

test('the main agent’s lane comes first and names its session, and a subagent’s lane names its type', () => {
  const shown = log([timed('scan', EXPLORE, 10, 60), timed('shape', MAIN, 20, 100)], { activity: [activity()] });
  const model = chart(shown, T0 + 600_000);
  assert.deepEqual(model.lanes.map((lane) => lane.label), ['main agent, s1', 'Explore']);
});

test('lanes that overlap in time are drawn one above the other', () => {
  const shown = log([timed('shape', MAIN, 0, 100), timed('scan', EXPLORE, 10, 60)], { activity: [activity()] });
  const drawn = gantt(shown, { width: 900, now: T0 + 600_000, onPick: () => {} });
  const tops = [...drawn.querySelectorAll('.lane-back')].map((rect) => Number(rect.getAttribute('y')));
  assert.equal(tops.length, 2);
  assert.ok(tops[1] > tops[0]);
});

test('each stage runs from the start of its work to its end, or to the start of its agent’s next stage', () => {
  const shown = log([timed('one', MAIN, 0, undefined), timed('two', MAIN, 50, 90)], { activity: [activity()] });
  const rows = chart(shown, T0 + 600_000).lanes[0].rows;
  assert.deepEqual(rows.map((row) => [row.stage.id, (row.stage.from - T0) / 1000, (row.stage.to - T0) / 1000]), [
    ['one', 0, 50],
    ['two', 50, 90],
  ]);
});

test('each task runs from its listing to its end, marked active from its start and done where it ended done', () => {
  const task = block('task', 't1', { title: 'Model it', state: 'done' }, { more: { times: { listed: at(5), started: at(10), ended: at(30) } } });
  const shown = log([timed('one', MAIN, 0, 60, [task])], { activity: [activity()] });
  const [drawnTask] = chart(shown, T0 + 600_000).lanes[0].rows[0].tasks;
  assert.deepEqual([(drawnTask.from - T0) / 1000, (drawnTask.activeFrom - T0) / 1000, (drawnTask.to - T0) / 1000, drawnTask.done], [5, 10, 30, true]);
  const svg = gantt(shown, { width: 900, now: T0 + 600_000, onPick: () => {} });
  assert.ok(svg.querySelector('.task-listed'));
  assert.ok(svg.querySelector('.task-done'));
});

test('a task with no recorded times is drawn at the time its current version was written', () => {
  const task = block('task', 't1', { title: 'Model it', state: 'open' }, { at: at(20) });
  const shown = log([timed('one', MAIN, 0, 60, [task])], { activity: [activity()] });
  const [drawnTask] = chart(shown, T0 + 600_000).lanes[0].rows[0].tasks;
  assert.equal(drawnTask.from, T0 + 20_000);
  assert.equal(drawnTask.to, T0 + 20_000);
});

test('each Loft run is drawn under the stage its agent had active, by its subcommands, and each Loft view at its time', () => {
  const recorded = block('loft', 'v1', { view: 'findings', recorded: { at: at(40) } });
  const shown = log([timed('one', MAIN, 0, 60, [recorded]), timed('two', MAIN, 60, 120)], {
    activity: [activity({ loft: [{ commands: ['check', 'fmt'], agent: 'main', ...span(30, 35), failed: false }] })],
  });
  const [one, two] = chart(shown, T0 + 600_000).lanes[0].rows;
  assert.deepEqual(one.loft.map((run) => run.name), ['check fmt']);
  assert.deepEqual(two.loft, []);
  assert.deepEqual(one.views.map((view) => [view.view, view.at - T0]), [['findings', 40_000]]);
});

test('each submission is drawn in its agent’s lane from its send to its answer', () => {
  const asked = block('question', 'q', { prompt: 'Which?', options: [] });
  asked.responses = [answer(['A'], 'page', at(80), 3)];
  const changes = [
    { what: 'sent', act: act(at(20), 2) },
    { what: 'answered', act: act(at(80), 3) },
  ];
  const sent = [{ key: 'sub', agent: MAIN, blocks: [asked.key], state: 'answered', changes }];
  const shown = log([timed('one', MAIN, 0, 100, [asked])], { activity: [activity()], sent });
  const [wait] = chart(shown, T0 + 600_000).lanes[0].rows[0].submissions;
  assert.deepEqual([(wait.from - T0) / 1000, (wait.to - T0) / 1000, wait.block], [20, 80, 'q']);
});

test('a block sent again in a later submission leaves the earlier submission ending at its own answer', () => {
  const plan = block('plan', 'p', { title: 'Plan', markdown: '', path: 'plan.md' });
  plan.responses = [answer(['approved'], 'page', at(30), 3), answer(['approved'], 'page', at(250), 5)];
  const submission = (key, from, to, revision) => ({
    key,
    agent: MAIN,
    blocks: [plan.key],
    state: 'answered',
    changes: [
      { what: 'sent', act: act(at(from), revision) },
      { what: 'answered', act: act(at(to), revision + 1) },
    ],
  });
  const sent = [submission('first', 20, 30, 2), submission('second', 200, 250, 4)];
  const shown = log([timed('one', MAIN, 0, 100, [plan]), timed('two', MAIN, 150, 300)], { activity: [activity()], sent });
  const [one, two] = chart(shown, T0 + 600_000).lanes[0].rows;
  const seconds = (wait) => [(wait.from - T0) / 1000, (wait.to - T0) / 1000];
  assert.deepEqual(one.submissions.map(seconds), [[20, 30]]);
  assert.deepEqual(two.submissions.map(seconds), [[200, 250]]);
});

test('a stage’s time in a turn with no active task, Loft run or wait is agent work', () => {
  const task = block('task', 't1', { title: 'T', state: 'done' }, { more: { times: { listed: at(0), started: at(10), ended: at(20) } } });
  const shown = log([timed('one', MAIN, 0, 60, [task])], {
    activity: [activity({ turns: [span(0, 50)], loft: [{ commands: ['check'], agent: 'main', ...span(30, 40) }] })],
  });
  const [row] = chart(shown, T0 + 600_000).lanes[0].rows;
  assert.deepEqual(row.work.map((each) => [(each.from - T0) / 1000, (each.to - T0) / 1000]), [
    [0, 10],
    [20, 30],
    [40, 50],
  ]);
});

test('time outside the turns is idle, and an idle stretch longer than 45 seconds is folded and labelled with its length', () => {
  const shown = log([timed('one', MAIN, 0, 400)], { activity: [activity({ turns: [span(0, 100), span(130, 200), span(300, 400)] })] });
  const model = chart(shown, T0 + 600_000);
  assert.deepEqual(model.idle.map((each) => [(each.to - each.from) / 1000, each.folded]), [
    [30, false],
    [100, true],
  ]);
  assert.ok(100_000 > FOLD_AFTER && 30_000 < FOLD_AFTER);
  const x = scale(model, 1000, 28);
  assert.equal(Math.round(x(T0 + 300_000) - x(T0 + 200_000)), 28);
  const drawn = gantt(shown, { width: 1000, now: T0 + 600_000, onPick: () => {} });
  assert.deepEqual([...drawn.querySelectorAll('.idle-label')].map((label) => label.textContent), ['idle 2 min']);
});

test('the bar above the chart divides the time not idle into tasks, Loft runs, waits and agent work, each with its length and share', () => {
  const task = block('task', 't1', { title: 'T', state: 'done' }, { more: { times: { listed: at(0), started: at(0), ended: at(20) } } });
  const asked = block('question', 'q', { prompt: 'Which?', options: [] });
  asked.responses = [answer(['A'], 'page', at(80), 3)];
  const sent = [{ key: 'sub', agent: MAIN, blocks: [asked.key], state: 'answered', changes: [{ what: 'sent', act: act(at(60), 2) }] }];
  const shown = log([timed('one', MAIN, 0, 100, [task, asked])], {
    sent,
    activity: [activity({ turns: [span(0, 100)], loft: [{ commands: ['check'], agent: 'main', ...span(30, 40) }] })],
  });
  const { summary } = chart(shown, T0 + 600_000);
  assert.deepEqual(summary.parts.map((part) => [part.kind, part.length / 1000, part.share]), [
    ['task', 20, 0.2],
    ['loft', 10, 0.1],
    ['wait', 20, 0.2],
    ['work', 50, 0.5],
  ]);
  const drawn = gantt(shown, { width: 900, now: T0 + 600_000, onPick: () => {} });
  assert.match(drawn.querySelector('.split-legend').textContent, /Active tasks: 20 s, 20%.*Loft runs: 10 s, 10%.*Waits on you: 20 s, 20%.*Agent work: 50 s, 50%/);
});

test('choosing a stage, a task or a submission in the chart opens the timeline at it', () => {
  const task = block('task', 't1', { title: 'T', state: 'done' }, { more: { times: { listed: at(0), started: at(0), ended: at(20) } } });
  const asked = block('question', 'q', { prompt: 'Which?', options: [] });
  const sent = [{ key: 'sub', agent: MAIN, blocks: [asked.key], state: 'open', changes: [{ what: 'sent', act: act(at(60), 2) }] }];
  const shown = log([timed('one', MAIN, 0, 100, [task, asked])], { sent, activity: [activity()] });
  const picked = [];
  const drawn = gantt(shown, { width: 900, now: T0 + 600_000, onPick: (id) => picked.push(id) });
  for (const selector of ['.stage-bar', '.task-bar', '.wait']) drawn.querySelector(selector).dispatchEvent(new Event('click'));
  assert.deepEqual(picked, ['one', 't1', 'q']);
});

test('each stretch of plan or spec mode is a band across every lane, labelled with its mode and length, an open one to now', () => {
  const shown = log([timed('one', MAIN, 0, 300), timed('scan', EXPLORE, 10, 60)], {
    activity: [activity({ plan: [span(10, 70)], spec: [span(200)] })],
  });
  const now = T0 + 260_000;
  const model = chart(shown, now);
  assert.deepEqual(model.bands.map((band) => [band.mode, (band.to - band.from) / 1000, band.open]), [
    ['plan', 60, false],
    ['spec', 60, true],
  ]);
  const drawn = gantt(shown, { width: 900, now, onPick: () => {} });
  const lanes = [...drawn.querySelectorAll('.lane-back')];
  const fills = [...drawn.querySelectorAll('.band-fill')];
  const bottom = Math.max(...lanes.map((lane) => Number(lane.getAttribute('y')) + Number(lane.getAttribute('height'))));
  for (const fill of fills) assert.ok(Number(fill.getAttribute('y')) + Number(fill.getAttribute('height')) >= bottom - 4, 'a band spans every lane');
  assert.deepEqual([...drawn.querySelectorAll('.band-label')].map((label) => label.textContent), ['Plan mode, 1 min', 'Spec mode, 1 min so far']);
  assert.equal(drawn.querySelector('.band-fill--spec').getAttribute('fill'), 'url(#hatch-spec)');
  assert.equal(drawn.querySelector('.band-fill--plan').getAttribute('fill'), null);
});

test('an idle stretch folded at the end of the chart is labelled inside it, its label ending at the chart’s edge', () => {
  const shown = log([timed('one', MAIN, 0, 3000)], { activity: [activity({ turns: [span(0, 400)] })] });
  const drawn = gantt(shown, { width: 1000, now: T0 + 3_000_000, onPick: () => {} });
  const label = [...drawn.querySelectorAll('.idle-label')].at(-1);
  assert.equal(label.textContent, 'idle 43 min');
  assert.equal(label.getAttribute('text-anchor'), 'end');
  assert.ok(Number(label.getAttribute('x')) <= 1000);
});
