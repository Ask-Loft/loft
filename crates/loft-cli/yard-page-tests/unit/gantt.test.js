// Yard Log along time: the Gantt chart's model and its drawing.

import { forget, root } from './dom.js';

import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';

import { FOLD_AFTER, chart, minimumWidth, place, scale, ticks } from '../../yard-page/model/gantt.js';
import { NAMES, columnWidth, gantt } from '../../yard-page/views/gantt.js';
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
  const lanes = [...drawn.querySelectorAll('.gantt-lane')];
  assert.deepEqual(lanes.map((lane) => lane.querySelector('.lane-label').textContent), ['main agent', 'Explore']);
  assert.ok(lanes[0].compareDocumentPosition(lanes[1]) & 4, 'the second lane follows the first, beneath it');
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

test('the name and the bar of a fix are drawn in a tone of their own, apart from those of a regular task', () => {
  const fix = block('task', 't1', { title: 'Mend it', state: 'open', type: 'fix' }, { more: { times: { listed: at(5), started: at(10) } } });
  const regular = block('task', 't2', { title: 'Model it', state: 'open' }, { more: { times: { listed: at(5), started: at(10) } } });
  const shown = log([timed('one', MAIN, 0, 60, [fix, regular])], { activity: [activity()] });
  assert.deepEqual(chart(shown, T0 + 600_000).lanes[0].rows[0].tasks.map((task) => task.fix), [true, false]);
  const svg = gantt(shown, { width: 900, now: T0 + 600_000, onPick: () => {} });
  const rows = [...svg.querySelectorAll('.task-row')];
  assert.deepEqual(rows.map((row) => row.classList.contains('task-row--fix')), [true, false]);
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
  const folds = [...drawn.querySelectorAll('.gantt-axis .idle-fold')];
  assert.deepEqual(folds.map((fold) => fold.getAttribute('title')), ['idle 2 min']);
  assert.ok(folds[0].querySelector('.break-mark'), 'the fold breaks the axis');
  assert.equal(drawn.querySelectorAll('.lane-overlay .idle-gap').length, 1);
});

test('a folded idle stretch is drawn apart from plan and spec mode, with no hatching', () => {
  const shown = log([timed('one', MAIN, 0, 400)], { activity: [activity({ turns: [span(0, 100), span(300, 400)], spec: [span(10, 90)] })] });
  const drawn = gantt(shown, { width: 1000, now: T0 + 600_000, onPick: () => {} });
  const parts = [...drawn.querySelectorAll('.idle-fold, .idle-gap')];
  assert.equal(parts.length, 2);
  for (const part of parts) {
    assert.ok(!/mode|band/.test(part.className), 'a fold is no band of a mode');
    assert.ok(!/gradient/.test(part.getAttribute('style') ?? ''), 'no hatching fills a fold');
  }
});

test('each session’s start in the main agent’s lane is a rule named by its number and time, giving its id on hover', () => {
  const later = { host: 'claude-code', session: 'second-session-id' };
  const shown = log([timed('shape', MAIN, 0, 100), timed('build', later, 110, 300)], { activity: [activity({ turns: [span(0, 300)] })] });
  const drawn = gantt(shown, { width: 1400, now: T0 + 600_000, onPick: () => {} });
  const rules = [...drawn.querySelectorAll('.session-rule')];
  assert.deepEqual(rules.map((rule) => rule.getAttribute('title')), ['Session 1: s1', 'Session 2: second-session-id']);
  const labels = [...drawn.querySelectorAll('.session-label')].map((label) => label.textContent);
  assert.equal(labels.length, 2);
  assert.match(labels[0], /^Session 1 · /);
  assert.match(labels[1], /^Session 2 · /);
});

test('each lane, stage and task is named in the column beside the chart, a task beneath its stage, each on the row of its bar', () => {
  const task = block('task', 't1', { title: 'Model it', state: 'done' }, { more: { times: { listed: at(5), started: at(10), ended: at(30) } } });
  const shown = log([timed('one', MAIN, 0, 60, [task]), timed('scan', EXPLORE, 10, 40)], { activity: [activity()] });
  const drawn = gantt(shown, { width: 1000, now: T0 + 600_000, onPick: () => {} });
  assert.deepEqual([...drawn.querySelectorAll('.lane-head .name-cell .lane-label')].map((label) => label.textContent), ['main agent', 'Explore']);
  const stage = drawn.querySelector('.stage-name');
  const named = drawn.querySelector('.task-name');
  assert.deepEqual([stage.textContent, named.textContent], ['one', 'Model it']);
  assert.ok(stage.closest('.name-cell') && named.closest('.name-cell'), 'names lie in the column');
  assert.ok(stage.closest('.gantt-row').querySelector('.track-cell .stage-bar'), 'a stage’s name is on its bar’s row');
  assert.ok(named.closest('.gantt-row').querySelector('.track-cell .task-bar'), 'a task’s name is on its bar’s row');
  assert.ok(stage.closest('.gantt-row').compareDocumentPosition(named.closest('.gantt-row')) & 4, 'a task’s row lies beneath its stage’s');
});

test('every name gives itself whole on hover and lies in the column, apart from the bars', () => {
  const long = 'A stage whose title runs far longer than any column of names could ever hold';
  const shown = log([timed(long, MAIN, 0, 60)], { activity: [activity()] });
  const drawn = gantt(shown, { width: 900, now: T0 + 600_000, onPick: () => {} });
  const name = drawn.querySelector('.stage-name');
  assert.equal(name.getAttribute('title'), long);
  assert.equal(name.textContent, long);
  for (const each of drawn.querySelectorAll('.lane-label, .stage-name, .task-name')) assert.ok(each.closest('.name-cell') && !each.closest('.track-cell'));
  for (const each of drawn.querySelectorAll('.stage-bar, .task-bar, .seg')) assert.ok(each.closest('.track-cell'));
});

test('each Loft run is drawn in its stage’s bar and names its subcommands on hover, and each Loft view is marked on the bar at its time', () => {
  const recorded = block('loft', 'v1', { view: 'findings', recorded: { at: at(40) } });
  const shown = log([timed('one', MAIN, 0, 60, [recorded])], {
    activity: [activity({ loft: [{ commands: ['check', 'fmt'], agent: 'main', ...span(30, 35), failed: false }] })],
  });
  const drawn = gantt(shown, { width: 900, now: T0 + 600_000, onPick: () => {} });
  const run = drawn.querySelector('.seg--loft');
  assert.equal(run.getAttribute('title'), 'loft check fmt');
  assert.ok(run.closest('.gantt-row').querySelector('.stage-bar'), 'the run lies on its stage’s bar');
  const mark = drawn.querySelector('.loft-view-mark');
  assert.equal(mark.getAttribute('title'), 'loft view: findings');
  assert.ok(mark.closest('.gantt-row').querySelector('.stage-bar'));
});

test('the bar above the chart divides the time not idle into Loft runs, subagent runs, tasks, waits and agent work, each with its length and share', () => {
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
    ['loft', 10, 0.1],
    ['subagent', 0, 0],
    ['task', 20, 0.2],
    ['wait', 20, 0.2],
    ['work', 50, 0.5],
  ]);
  const drawn = gantt(shown, { width: 900, now: T0 + 600_000, onPick: () => {} });
  assert.match(drawn.querySelector('.split-legend').textContent, /Loft runs: 10 s, 10%.*Subagent runs: 0 s, 0%.*Active tasks: 20 s, 20%.*Waits on you: 20 s, 20%.*Agent work: 50 s, 50%/);
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
  const lanes = [...drawn.querySelectorAll('.gantt-lane')];
  assert.equal(lanes.length, 2);
  for (const lane of lanes) {
    const overlay = lane.querySelector(':scope > .lane-overlay');
    assert.ok(overlay.querySelector('.band-fill--plan') && overlay.querySelector('.band-fill--spec'), 'each band shades every lane');
  }
  const track = [...drawn.querySelectorAll('.gantt-head .mode-track .mode')];
  assert.deepEqual(track.map((mode) => mode.getAttribute('title')), ['Plan mode, 1 min', 'Spec mode, 1 min so far']);
  assert.deepEqual(track.map((mode) => mode.className), ['mode mode--plan', 'mode mode--spec']);
  assert.deepEqual([...drawn.querySelectorAll('.band-label')].map((label) => label.textContent), ['Plan mode', 'Spec mode']);
});

test('a label of the track of modes is drawn only where it fits within its stretch', () => {
  const shown = log([timed('one', MAIN, 0, 600)], { activity: [activity({ plan: [span(10, 14), span(100, 400)] })] });
  const drawn = gantt(shown, { width: 900, now: T0 + 600_000, onPick: () => {} });
  const modes = [...drawn.querySelectorAll('.mode')];
  assert.deepEqual(modes.map((mode) => mode.querySelector('.band-label')?.textContent ?? null), [null, 'Plan mode']);
});

test('the axis’s first label begins at the chart’s start and a label at its end ends there, and no label crosses a break', () => {
  const shown = log([timed('one', MAIN, 0, 3600)], { activity: [activity({ turns: [span(0, 1500), span(2400, 3600)] })] });
  const drawn = gantt(shown, { width: 1200, now: T0 + 3_600_000, onPick: () => {} });
  const ticks = [...drawn.querySelectorAll('.gantt-axis .tick')];
  assert.ok(ticks.length >= 3);
  assert.equal(ticks[0].className, 'tick tick--start');
  assert.equal(ticks.at(-1).className, 'tick tick--end');
  assert.ok(ticks.some((tick) => tick.className === 'tick tick--start' && tick !== ticks[0]), 'time resuming after a break is labelled from the break');
});

test('each time lies at its share of the track’s width and its offset in pixels, so a bar follows the width the browser gives it', () => {
  const shown = log([timed('one', MAIN, 100, 400)], { activity: [activity({ turns: [span(0, 200), span(300, 600)] })] });
  const model = chart(shown, T0 + 600_000);
  const at = place(model, 28);
  const drawn = gantt(shown, { width: 1000, now: T0 + 600_000, onPick: () => {} });
  const style = drawn.querySelector('.stage-bar').getAttribute('style');
  const [, share, sign, pixels] = /left: calc\(([\d.]+)% ([+-]) ([\d.]+)px\)/.exec(style) ?? [];
  const start = at(T0 + 100_000);
  assert.ok(Math.abs(Number(share) - start.share * 100) < 0.001);
  assert.ok(Math.abs((sign === '-' ? -1 : 1) * Number(pixels) - start.offset) < 0.01);
  for (const width of [500, 1000, 2000]) {
    const x = scale(model, width, 28);
    assert.ok(Math.abs(at(T0 + 450_000).share * width + at(T0 + 450_000).offset - x(T0 + 450_000)) < 1e-6);
  }
});

test('a minute that is not folded is never narrower than 8 pixels, the sheet never narrower than its time needs', () => {
  const shown = log([timed('one', MAIN, 0, 600)], { activity: [activity({ turns: [span(0, 600)] })] });
  const model = chart(shown, T0 + 600_000);
  assert.equal(minimumWidth(model, 28), 80);
  const folded = chart(log([timed('one', MAIN, 0, 900)], { activity: [activity({ turns: [span(0, 300), span(600, 900)] })] }), T0 + 900_000);
  assert.equal(minimumWidth(folded, 28), 80 + 28);
  const drawn = gantt(shown, { width: 900, now: T0 + 600_000, onPick: () => {} });
  assert.match(drawn.querySelector('.gantt-sheet').getAttribute('style'), /width: max\(100%, calc\(var\(--names\) \+ 80px\)\)/);
});

test('a faint grid runs behind the bars: a line at each mark of the time axis in every lane, and a line above each stage’s rows', () => {
  const task = block('task', 't1', { title: 'T', state: 'done' }, { more: { times: { listed: at(0), started: at(0), ended: at(20) } } });
  const shown = log([timed('one', MAIN, 0, 1800, [task]), timed('two', MAIN, 1800, 3600), timed('scan', EXPLORE, 10, 60)], {
    activity: [activity({ turns: [span(0, 3600)] })],
  });
  const drawn = gantt(shown, { width: 1200, now: T0 + 3_600_000, onPick: () => {} });
  const marks = drawn.querySelectorAll('.gantt-axis .tick').length;
  assert.ok(marks >= 2);
  for (const lane of drawn.querySelectorAll('.gantt-lane')) assert.equal(lane.querySelectorAll('.lane-overlay .grid-line').length, marks);
  assert.equal(drawn.querySelectorAll('.stage-row').length, 3);
  assert.equal(drawn.querySelectorAll('.task-row').length, 1);
});

test('the column’s edge moves the column by the length dragged, within 160 and 480 pixels', () => {
  assert.equal(columnWidth(260, 40), 300);
  assert.equal(columnWidth(260, -500), NAMES.least);
  assert.equal(columnWidth(260, 500), NAMES.most);
  assert.deepEqual([NAMES.least, NAMES.most], [160, 480]);
});

test('dragging the column’s edge sets its width, and the width chosen is the one the next chart opens with', () => {
  const shown = log([timed('one', MAIN, 0, 60)], { activity: [activity()] });
  const holder = root();
  holder.append(gantt(shown, { width: 1000, now: T0 + 600_000, onPick: () => {} }));
  assert.match(holder.querySelector('.gantt-frame').getAttribute('style'), /--names: 260px/);
  const handle = holder.querySelector('.names-edge');
  const pointer = (type, clientX) => Object.assign(new Event(type), { clientX, pointerId: 1 });
  handle.dispatchEvent(pointer('pointerdown', 500));
  handle.dispatchEvent(pointer('pointermove', 600));
  assert.match(holder.querySelector('.gantt-frame').getAttribute('style'), /--names: 360px/);
  handle.dispatchEvent(pointer('pointerup', 600));
  assert.equal(localStorage.getItem('yard.gantt-names'), '360');
  const again = gantt(shown, { width: 1000, now: T0 + 600_000, onPick: () => {} });
  assert.match(again.querySelector('.gantt-frame').getAttribute('style'), /--names: 360px/);
  const edge = holder.querySelector('.names-edge');
  edge.dispatchEvent(Object.assign(new Event('keydown'), { key: 'ArrowLeft' }));
  assert.equal(localStorage.getItem('yard.gantt-names'), '344');
});

/** A run of subagent `agent`, of type `type`, from `from` to `to` seconds, for the stage `stage`. */
function run(agent, type, from, to, stage) {
  return { agent, type, ...span(from, to), ...(stage ? { stage: stage.key } : {}) };
}

test('the run of a subagent that owned no stage is drawn in the bar of the stage it worked for, to now while it runs, naming its type on hover', () => {
  const one = timed('one', MAIN, 0, 60);
  const two = timed('two', MAIN, 60, undefined, [], 'active');
  const shown = log([one, two], { activity: [activity({ subagents: [run('a2', 'Explore', 10, 30, one), run('a3', 'Plan', 70, undefined, two)] })] });
  const model = chart(shown, T0 + 150_000);
  assert.deepEqual(model.lanes.map((lane) => lane.label), ['main agent'], 'a subagent that owned no stage has no lane');
  const [first, second] = model.lanes[0].rows;
  const seconds = (each) => [each.type, (each.from - T0) / 1000, (each.to - T0) / 1000];
  assert.deepEqual(first.subagents.map(seconds), [['Explore', 10, 30]]);
  assert.deepEqual(second.subagents.map(seconds), [['Plan', 70, 150]]);
  assert.deepEqual(first.segments.filter((each) => each.kind === 'subagent').map((each) => [(each.from - T0) / 1000, (each.to - T0) / 1000]), [[10, 30]]);
  const drawn = gantt(shown, { width: 900, now: T0 + 150_000, onPick: () => {} });
  const titles = [...drawn.querySelectorAll('.seg--subagent')].map((each) => each.getAttribute('title'));
  assert.deepEqual(titles, ['Explore subagent for 20 s', 'Plan subagent for 1 min']);
});

test('the run of a subagent that owned a stage is drawn in its own lane and in no other stage’s bar', () => {
  const one = timed('one', MAIN, 0, 100);
  const shown = log([one, timed('scan', EXPLORE, 20, 50)], { activity: [activity({ subagents: [run('a1', 'Explore', 10, 50, one)] })] });
  const model = chart(shown, T0 + 600_000);
  assert.deepEqual(model.lanes.map((lane) => lane.label), ['main agent', 'Explore']);
  const [main] = model.lanes[0].rows;
  assert.deepEqual(main.subagents, []);
  assert.ok(main.segments.every((each) => each.kind !== 'subagent'));
});

test('time inside a subagent’s run but outside the main agent’s turns is not idle', () => {
  const one = timed('one', MAIN, 0, 300);
  const shown = log([one], { activity: [activity({ turns: [span(0, 100)], subagents: [run('a2', 'Explore', 100, 200, one)] })] });
  const model = chart(shown, T0 + 300_000);
  assert.deepEqual(model.idle.map((each) => [(each.from - T0) / 1000, (each.to - T0) / 1000]), [[200, 300]]);
  const [row] = model.lanes[0].rows;
  assert.deepEqual(row.segments.map((each) => [each.kind, (each.from - T0) / 1000, (each.to - T0) / 1000]), [
    ['work', 0, 100],
    ['subagent', 100, 200],
  ]);
});

test('a stage’s bar counts each moment to its Loft runs, then its subagents’ runs, then its tasks, waits and work', () => {
  const task = block('task', 't1', { title: 'T', state: 'done' }, { more: { times: { listed: at(0), started: at(10), ended: at(40) } } });
  const one = timed('one', MAIN, 0, 60, [task]);
  const shown = log([one], {
    activity: [activity({ turns: [span(0, 60)], loft: [{ commands: ['check'], agent: 'main', ...span(30, 35) }], subagents: [run('a2', 'Explore', 20, 50, one)] })],
  });
  const model = chart(shown, T0 + 600_000);
  const [row] = model.lanes[0].rows;
  assert.deepEqual(row.segments.map((each) => [each.kind, (each.from - T0) / 1000, (each.to - T0) / 1000]), [
    ['work', 0, 10],
    ['task', 10, 20],
    ['subagent', 20, 30],
    ['loft', 30, 35],
    ['subagent', 35, 50],
    ['work', 50, 60],
  ]);
  const counted = (kind) => row.segments.filter((each) => each.kind === kind).reduce((sum, each) => sum + each.to - each.from, 0);
  for (const part of model.summary.parts) assert.equal(counted(part.kind), part.length, part.kind);
});

test('a Loft run of a subagent that owned no stage is drawn in the bar of the stage it counted toward', () => {
  const one = timed('one', MAIN, 0, 60);
  const shown = log([one, timed('two', MAIN, 60, 120)], {
    activity: [activity({ loft: [{ commands: ['check'], agent: 'a2', ...span(70, 75), failed: false, stage: one.key }] })],
  });
  const [first, second] = chart(shown, T0 + 600_000).lanes[0].rows;
  assert.deepEqual(first.loft.map((each) => each.name), ['check']);
  assert.deepEqual(second.loft, []);
});
