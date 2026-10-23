// Yard Log along time: the Gantt chart's model and its drawing.

import { forget } from './dom.js';

import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';

import { FOLD_AFTER, chart, scale, ticks } from '../../yard-page/model/gantt.js';
import { CHARACTER, gantt } from '../../yard-page/views/gantt.js';
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

test('the main agent’s lane comes first, and a subagent’s lane names its type', () => {
  const shown = log([timed('scan', EXPLORE, 10, 60), timed('shape', MAIN, 20, 100)], { activity: [activity()] });
  const model = chart(shown, T0 + 600_000);
  assert.deepEqual(model.lanes.map((lane) => lane.label), ['main agent', 'Explore']);
});

test('the main agent’s lane holds the stages of every session, numbering the sessions in the order they started', () => {
  const later = { host: 'claude-code', session: 's2' };
  const shown = log([timed('shape', MAIN, 0, 100), timed('build', later, 200, 300), timed('check', MAIN, 120, 150)], {
    activity: [activity({ turns: [span(0, 150)] }), activity({ session: 's2', turns: [span(200, 300)] })],
  });
  const [lane, ...others] = chart(shown, T0 + 600_000).lanes;
  assert.equal(others.length, 0);
  assert.deepEqual(lane.rows.map((row) => row.stage.id), ['shape', 'check', 'build']);
  assert.deepEqual(lane.sessions.map((each) => [each.session, each.number, (each.from - T0) / 1000]), [
    ['s1', 1, 0],
    ['s2', 2, 200],
  ]);
});

test('a stage’s bar is divided into its tasks, Loft runs, waits and work as the bar above the chart counts them, idle time left empty', () => {
  const task = block('task', 't1', { title: 'T', state: 'done' }, { more: { times: { listed: at(0), started: at(10), ended: at(40) } } });
  const asked = block('question', 'q', { prompt: 'Which?', options: [] });
  const changes = [
    { what: 'sent', act: act(at(35), 2) },
    { what: 'answered', act: act(at(60), 3) },
  ];
  const sent = [{ key: 'sub', agent: MAIN, blocks: [asked.key], state: 'answered', changes }];
  const shown = log([timed('one', MAIN, 0, 100, [task, asked])], {
    sent,
    activity: [activity({ turns: [span(0, 70), span(90, 100)], loft: [{ commands: ['check'], agent: 'main', ...span(30, 38) }] })],
  });
  const model = chart(shown, T0 + 600_000);
  const [row] = model.lanes[0].rows;
  assert.deepEqual(row.segments.map((each) => [each.kind, (each.from - T0) / 1000, (each.to - T0) / 1000]), [
    ['work', 0, 10],
    ['task', 10, 30],
    ['loft', 30, 38],
    ['task', 38, 40],
    ['wait', 40, 60],
    ['work', 60, 70],
    ['work', 90, 100],
  ]);
  assert.equal(row.segments.find((each) => each.kind === 'wait').block, 'q');
  const counted = (kind) => row.segments.filter((each) => each.kind === kind).reduce((sum, each) => sum + each.to - each.from, 0);
  for (const part of model.summary.parts) assert.equal(counted(part.kind), part.length, part.kind);
});

test('the time axis is marked at regular times within each stretch between folded idle stretches, no two marks crowding', () => {
  const shown = log([timed('one', MAIN, 0, 3600)], { activity: [activity({ turns: [span(0, 1200), span(2400, 3600)] })] });
  const model = chart(shown, T0 + 4_000_000);
  const x = scale(model, 1000, 28);
  const marks = ticks(model, x, 72);
  const inner = marks.filter((time) => time !== model.start && time !== model.end);
  assert.ok(inner.length >= 4, 'each stretch has marks of its own');
  const steps = new Set(inner.map((time) => (time - T0) % 300_000));
  assert.deepEqual([...steps], [0], 'marks fall on whole steps');
  assert.ok(inner.every((time) => time <= T0 + 1_200_000 || time >= T0 + 2_400_000), 'no mark inside a folded stretch');
  for (let index = 1; index < marks.length; index += 1) assert.ok(x(marks[index]) - x(marks[index - 1]) >= 72);
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

test('each Loft run belongs to the stage its agent had active, by its subcommands, and each Loft view to its time', () => {
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

test('time outside the turns is idle, and an idle stretch longer than 45 seconds is folded to a break in the axis that gives its length on hover', () => {
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
  const folds = [...drawn.querySelectorAll('.idle-fold')];
  assert.deepEqual(folds.map((fold) => fold.querySelector('title').textContent), ['idle 2 min']);
  assert.ok(folds[0].querySelector('.break-mark'), 'the fold breaks the axis');
});

test('a folded idle stretch is drawn apart from plan and spec mode, with no hatching', () => {
  const shown = log([timed('one', MAIN, 0, 400)], { activity: [activity({ turns: [span(0, 100), span(300, 400)], spec: [span(10, 90)] })] });
  const drawn = gantt(shown, { width: 1000, now: T0 + 600_000, onPick: () => {} });
  const fold = drawn.querySelector('.idle-fold');
  assert.ok(fold);
  for (const part of fold.querySelectorAll('*')) assert.ok(!String(part.getAttribute('fill') ?? '').startsWith('url('), 'no pattern fills a fold');
  assert.equal(drawn.querySelector('#hatch-idle'), null);
});

test('a session’s start in the main agent’s lane is a rule named by its number and time, giving its id on hover', () => {
  const later = { host: 'claude-code', session: 'second-session-id' };
  const shown = log([timed('shape', MAIN, 0, 100), timed('build', later, 110, 300)], { activity: [activity({ turns: [span(0, 300)] })] });
  const drawn = gantt(shown, { width: 1200, now: T0 + 600_000, onPick: () => {} });
  const sessions = [...drawn.querySelectorAll('.session')];
  assert.equal(sessions.length, 1, 'the first session needs no rule');
  assert.ok(sessions[0].querySelector('.session-rule'));
  assert.match(sessions[0].querySelector('.session-label').textContent, /^Session 2 · /);
  assert.equal(sessions[0].querySelector('title').textContent, 'Session 2: second-session-id');
});

test('each lane, stage and task is named in the column beside the chart, a task indented under its stage, each level with its bar', () => {
  const task = block('task', 't1', { title: 'Model it', state: 'done' }, { more: { times: { listed: at(5), started: at(10), ended: at(30) } } });
  const shown = log([timed('one', MAIN, 0, 60, [task]), timed('scan', EXPLORE, 10, 40)], { activity: [activity()] });
  const drawn = gantt(shown, { width: 1000, now: T0 + 600_000, onPick: () => {} });
  assert.deepEqual([...drawn.querySelectorAll('.lane-label')].map((label) => label.textContent), ['main agent', 'Explore']);
  const stage = drawn.querySelector('.stage-name text');
  const named = drawn.querySelector('.task-name text');
  assert.deepEqual([stage.textContent, named.textContent], ['one', 'Model it']);
  assert.ok(Number(named.getAttribute('x')) > Number(stage.getAttribute('x')), 'a task is indented under its stage');
  const middle = (rect) => Number(rect.getAttribute('y')) + Number(rect.getAttribute('height')) / 2;
  assert.ok(Math.abs(Number(stage.getAttribute('y')) - 4 - middle(drawn.querySelector('.stage-rect'))) <= 2, 'a stage’s name is level with its bar');
  assert.ok(Math.abs(Number(named.getAttribute('y')) - 4 - middle(drawn.querySelector('.task-listed'))) <= 2, 'a task’s name is level with its bar');
  assert.ok(Number(named.getAttribute('y')) > Number(stage.getAttribute('y')), 'a task’s row lies beneath its stage’s');
});

test('a name longer than the column is cut short with an ellipsis and given whole on hover, and no name is drawn over the bars', () => {
  const long = 'A stage whose title runs far longer than any column of names could ever hold';
  const shown = log([timed(long, MAIN, 0, 60)], { activity: [activity()] });
  const drawn = gantt(shown, { width: 900, now: T0 + 600_000, onPick: () => {} });
  const name = drawn.querySelector('.stage-name');
  assert.match(name.querySelector('text').textContent, /…$/);
  assert.equal(name.querySelector('title').textContent, long);
  const bars = Math.min(...[...drawn.querySelectorAll('.stage-rect')].map((rect) => Number(rect.getAttribute('x'))));
  for (const text of drawn.querySelectorAll('.lane-label, .stage-name text, .task-name text')) {
    assert.ok(Number(text.getAttribute('x')) + text.textContent.length * CHARACTER <= bars, `${text.textContent} stays in the column`);
  }
});

test('each Loft run is drawn in its stage’s bar and names its subcommands on hover, and each Loft view is marked on the bar at its time', () => {
  const recorded = block('loft', 'v1', { view: 'findings', recorded: { at: at(40) } });
  const shown = log([timed('one', MAIN, 0, 60, [recorded])], {
    activity: [activity({ loft: [{ commands: ['check', 'fmt'], agent: 'main', ...span(30, 35), failed: false }] })],
  });
  const drawn = gantt(shown, { width: 900, now: T0 + 600_000, onPick: () => {} });
  const run = drawn.querySelector('.seg--loft');
  const bar = drawn.querySelector('.stage-rect');
  assert.equal(run.getAttribute('y'), bar.getAttribute('y'));
  assert.equal(run.querySelector('title').textContent, 'loft check fmt');
  const mark = drawn.querySelector('.loft-view-mark');
  assert.equal(mark.querySelector('title').textContent, 'loft view: findings');
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
  for (const selector of ['.stage-bar', '.stage-name', '.task-bar', '.task-name', '.wait']) drawn.querySelector(selector).dispatchEvent(new Event('click'));
  assert.deepEqual(picked, ['one', 'one', 't1', 't1', 'q']);
});

test('each stretch of plan or spec mode is drawn in the track of modes and shaded over every lane, labelled with its mode and its length on hover', () => {
  const shown = log([timed('one', MAIN, 0, 300), timed('scan', EXPLORE, 10, 60)], {
    activity: [activity({ plan: [span(10, 70)], spec: [span(200)] })],
  });
  const now = T0 + 260_000;
  const model = chart(shown, now);
  assert.deepEqual(model.bands.map((band) => [band.mode, (band.to - band.from) / 1000, band.open]), [
    ['plan', 60, false],
    ['spec', 60, true],
  ]);
  const drawn = gantt(shown, { width: 1200, now, onPick: () => {} });
  const lanes = [...drawn.querySelectorAll('.lane-back')];
  const fills = [...drawn.querySelectorAll('.band-fill')];
  const top = Math.min(...lanes.map((lane) => Number(lane.getAttribute('y'))));
  const bottom = Math.max(...lanes.map((lane) => Number(lane.getAttribute('y')) + Number(lane.getAttribute('height'))));
  for (const fill of fills) {
    assert.ok(Number(fill.getAttribute('y')) <= top && Number(fill.getAttribute('y')) + Number(fill.getAttribute('height')) >= bottom, 'a band spans every lane');
    for (const lane of lanes) assert.ok(lane.compareDocumentPosition(fill) & 4, 'a band is drawn over each lane’s background');
  }
  const track = [...drawn.querySelectorAll('.mode')];
  assert.deepEqual(track.map((mode) => mode.querySelector('title').textContent), ['Plan mode, 1 min', 'Spec mode, 1 min so far']);
  for (const mode of track) assert.ok(Number(mode.querySelector('rect').getAttribute('y')) + Number(mode.querySelector('rect').getAttribute('height')) <= top, 'the track lies above the lanes');
  assert.deepEqual([...drawn.querySelectorAll('.band-label')].map((label) => label.textContent), ['Plan mode', 'Spec mode']);
  assert.equal(drawn.querySelector('.band-fill--spec').getAttribute('fill'), 'url(#hatch-spec)');
  assert.equal(drawn.querySelector('.mode--spec rect').getAttribute('fill'), 'url(#hatch-spec)');
  assert.equal(drawn.querySelector('.band-fill--plan').getAttribute('fill'), null);
});

test('a label of the track of modes is drawn only where it fits within its stretch', () => {
  const shown = log([timed('one', MAIN, 0, 600)], { activity: [activity({ plan: [span(10, 14), span(100, 400)] })] });
  const drawn = gantt(shown, { width: 900, now: T0 + 600_000, onPick: () => {} });
  const modes = [...drawn.querySelectorAll('.mode')];
  assert.deepEqual(modes.map((mode) => mode.querySelector('.band-label')?.textContent ?? null), [null, 'Plan mode']);
  const label = modes[1].querySelector('.band-label');
  const rect = modes[1].querySelector('rect');
  assert.ok(Number(label.getAttribute('x')) + label.textContent.length * CHARACTER <= Number(rect.getAttribute('x')) + Number(rect.getAttribute('width')));
});

test('no label of the chart runs past its edges', () => {
  const later = { host: 'claude-code', session: 's2' };
  const shown = log([timed('A first stage with a long title', MAIN, 0, 400), timed('The last stage, also with a long title', later, 2400, 3000)], {
    activity: [activity({ turns: [span(0, 400), span(2400, 3000)], plan: [span(2500, 2990)], spec: [span(0, 50)] })],
  });
  for (const width of [480, 900, 1600]) {
    const drawn = gantt(shown, { width, now: T0 + 3_000_000, onPick: () => {} });
    for (const text of drawn.querySelectorAll('svg text')) {
      const x = Number(text.getAttribute('x'));
      const extent = text.textContent.length * CHARACTER;
      const anchor = text.getAttribute('text-anchor') ?? 'start';
      const [left, right] = anchor === 'end' ? [x - extent, x] : anchor === 'middle' ? [x - extent / 2, x + extent / 2] : [x, x + extent];
      assert.ok(left >= 0 && right <= width, `“${text.textContent}” lies inside a chart ${width} wide`);
    }
  }
});
