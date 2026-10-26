// Yard Log along time, as the Gantt chart draws it: a lane for each agent that owned a stage,
// each lane holding its stages with their tasks, Loft runs, Loft views, the runs of the subagents
// that owned no stage and worked for them, waits on the person and the agent's own work between
// them; the idle time outside the main agent's turns and the subagents' runs, folded where it is
// long; the stretches of plan and spec mode; and where the time that was not idle went.

import { agentKey, ms, who } from '../lib/format.js';
import { blocksByKey } from './log.js';

/** Idle time longer than this, in milliseconds, is folded to a fixed width. */
export const FOLD_AFTER = 45_000;

/**
 * @typedef {{ from: number, to: number }} Span
 */

/**
 * The spans of `spans` merged where they touch or overlap, in order.
 * @param {Span[]} spans
 * @returns {Span[]}
 */
export function union(spans) {
  const sorted = spans.filter((span) => span.to > span.from).sort((a, b) => a.from - b.from);
  /** @type {Span[]} */
  const merged = [];
  for (const span of sorted) {
    const last = merged[merged.length - 1];
    if (last && span.from <= last.to) last.to = Math.max(last.to, span.to);
    else merged.push({ ...span });
  }
  return merged;
}

/**
 * What of `spans` lies outside `cut`.
 * @param {Span[]} spans
 * @param {Span[]} cut
 * @returns {Span[]}
 */
export function subtract(spans, cut) {
  let left = union(spans);
  for (const hole of union(cut)) {
    left = left.flatMap((span) => {
      if (hole.to <= span.from || hole.from >= span.to) return [span];
      const parts = [];
      if (hole.from > span.from) parts.push({ from: span.from, to: hole.from });
      if (hole.to < span.to) parts.push({ from: hole.to, to: span.to });
      return parts;
    });
  }
  return left;
}

/**
 * What of `spans` lies inside `within`.
 * @param {Span[]} spans
 * @param {Span[]} within
 * @returns {Span[]}
 */
export function intersect(spans, within) {
  const outside = subtract(spans, within);
  return subtract(spans, outside);
}

/** @param {Span[]} spans */
export function measure(spans) {
  return union(spans).reduce((sum, span) => sum + span.to - span.from, 0);
}

/**
 * The Gantt chart of `log` at `now`.
 * @param {any} log
 * @param {number} now
 */
export function chart(log, now) {
  const sessions = activities(log, now);
  // Time is busy while the main agent's turns or any subagent's run go on.
  const turns = union(sessions.flatMap((activity) => [...activity.turns, ...activity.subagents]));
  const waits = submissionSpans(log, now);
  const lanes = laneList(log, now, sessions, turns, waits);
  const drawn = lanes.flatMap((lane) =>
    lane.rows.flatMap((row) => [row.stage, ...row.tasks, ...row.loft, ...row.subagents, ...row.submissions]),
  );
  const starts = [...drawn.map((span) => span.from), ...turns.map((turn) => turn.from)];
  const ends = [...drawn.map((span) => span.to), ...turns.map((turn) => turn.to)];
  const start = starts.length ? Math.min(...starts) : now;
  const end = ends.length ? Math.max(...ends, start + 1000) : now;
  const idle = turns.length ? subtract([{ from: start, to: end }], turns) : [];
  const bands = sessions.flatMap((activity) => activity.bands);
  return {
    start,
    end,
    lanes,
    idle: idle.map((span) => ({ ...span, folded: span.to - span.from > FOLD_AFTER })),
    bands,
    summary: summary(turns.length ? turns : [{ from: start, to: end }], lanes),
  };
}

/**
 * Each session's activity with its times in milliseconds, a stretch still going on ending now.
 * @param {any} log
 * @param {number} now
 */
function activities(log, now) {
  const span = (item) => ({
    from: ms(item.started),
    to: item.ended ? ms(item.ended) : now,
  });
  return (log?.activity ?? []).map((activity) => ({
    session: activity.session,
    turns: (activity.turns ?? []).map((turn) => span(turn)),
    bands: [
      ...(activity.plan ?? []).map((item) => ({ mode: 'plan', ...span(item), open: !item.ended })),
      ...(activity.spec ?? []).map((item) => ({ mode: 'spec', ...span(item), open: !item.ended })),
    ],
    subagents: (activity.subagents ?? []).map((run) => ({
      ...span(run),
      agent: agentKey({ host: activity.host, session: activity.session, agent: run.agent }),
      type: run.type ?? 'subagent',
      stage: run.stage,
    })),
    loft: (activity.loft ?? []).map((run) => ({
      ...span(run),
      agent: agentKey({ host: activity.host, session: activity.session, ...(run.agent === 'main' ? {} : { agent: run.agent }) }),
      stage: run.stage,
      name: (run.commands ?? []).join(' '),
      failed: run.failed === true,
    })),
  }));
}

/**
 * Each submission's span, from its send to its own answer, its withdrawal, or now. A block sent
 * again in a later submission holds the later answers too, so a submission's end is its own
 * `answered` change, and its blocks' answers only where it records none.
 * @param {any} log
 * @param {number} now
 */
function submissionSpans(log, now) {
  const blocks = blocksByKey(log);
  return (log?.sent ?? []).map((submission) => {
    const sent = submission.changes.find((change) => change.what === 'sent');
    const answered = submission.changes.find((change) => change.what === 'answered');
    const withdrawn = submission.changes.find((change) => change.what === 'withdrawn');
    const answers = submission.blocks
      .flatMap((key) => blocks.get(key)?.responses ?? [])
      .map((response) => ms(response.act?.at));
    let to = now;
    if (submission.state === 'withdrawn' && withdrawn) to = ms(withdrawn.act.at);
    else if (submission.state === 'answered' && answered) to = ms(answered.act.at);
    else if (submission.state === 'answered' && answers.length) to = Math.max(...answers);
    return {
      key: submission.key,
      agent: agentKey(submission.agent),
      from: ms(sent?.act.at),
      to,
      open: submission.state === 'open',
      block: blocks.get(submission.blocks[0])?.id ?? null,
    };
  });
}

/**
 * The lanes: one for the main agent, holding the stages of every session with the time each
 * session first started work, then one for each subagent that owned a stage, in the order each
 * first started work. The run of a subagent that owned no stage lies in a stage's row.
 * @param {any} log
 * @param {number} now
 * @param {ReturnType<typeof activities>} sessions
 * @param {Span[]} turns
 * @param {ReturnType<typeof submissionSpans>} waits
 */
function laneList(log, now, sessions, turns, waits) {
  /** @type {Map<string, { key: string, label: string, main: boolean, stages: any[] }>} */
  const lanes = new Map();
  for (const stage of log?.stages ?? []) {
    const started = stage.times?.started;
    if (!started || !stage.owner) continue;
    const main = !stage.owner.agent;
    const key = main ? 'main' : agentKey(stage.owner);
    const lane = lanes.get(key) ?? { key, label: main ? 'main agent' : who(stage.owner), main, stages: [] };
    lane.stages.push(stage);
    lanes.set(key, lane);
  }
  const spans = {
    loft: sessions.flatMap((activity) => activity.loft),
    subagents: sessions.flatMap((activity) => activity.subagents).filter((run) => !lanes.has(run.agent)),
    waits,
    turns,
  };
  const built = [...lanes.values()].map((lane) => {
    const stages = lane.stages.sort((a, b) => ms(a.times.started) - ms(b.times.started));
    const rows = stages.map((stage, index) => {
      const next = stages[index + 1];
      const from = ms(stage.times.started);
      let to = stage.times.ended ? ms(stage.times.ended) : next ? ms(next.times.started) : now;
      if (to < from) to = from;
      return row(stage, { from, to }, agentKey(stage.owner), spans, now);
    });
    return { key: lane.key, label: lane.label, main: lane.main, sessions: lane.main ? sessionStarts(stages) : [], rows };
  });
  const first = (lane) => Math.min(...lane.rows.map((row) => row.stage.from));
  return built.sort((a, b) => Number(b.main) - Number(a.main) || first(a) - first(b));
}

/**
 * Each session of the main agent with the time its first stage started, numbered in that order.
 * @param {any[]} stages the main agent's stages, in the order they started
 */
function sessionStarts(stages) {
  /** @type {Map<string, number>} */
  const starts = new Map();
  for (const stage of stages) {
    const session = agentKey({ host: stage.owner.host, session: stage.owner.session });
    if (!starts.has(session)) starts.set(session, ms(stage.times.started));
  }
  return [...starts].map(([session, from], index) => ({ session, number: index + 1, from }));
}

/**
 * Where the busy time went, each moment counted once, to the first it falls in of the Loft
 * runs, the subagents' runs, the active tasks and the waits on the person, and the rest to the
 * agent's own work.
 * @param {Span[]} busy
 * @param {{ loft: Span[], subagents: Span[], tasks: Span[], waits: Span[] }} spans
 */
function split(busy, { loft, subagents, tasks, waits }) {
  const ran = intersect(loft, busy);
  const helped = subtract(intersect(subagents, busy), ran);
  const active = subtract(intersect(tasks, busy), [...ran, ...helped]);
  const waited = subtract(intersect(waits, busy), [...ran, ...helped, ...active]);
  const work = subtract(busy, [...ran, ...helped, ...active, ...waited]);
  return { loft: ran, subagent: helped, task: active, wait: waited, work };
}

/**
 * One stage's row: its bar, its tasks, the Loft runs and views made while it was active, the runs
 * of the subagents that owned no stage and worked for it, its agent's waits on the person, the
 * agent's own work in the rest of its time, and its bar's time divided between them.
 * @param {any} stage
 * @param {Span} span
 * @param {string} agent
 * @param {{ loft: any[], subagents: any[], waits: any[], turns: Span[] }} spans
 * @param {number} now
 */
function row(stage, span, agent, { loft, subagents, waits, turns }, now) {
  const tasks = (stage.blocks ?? [])
    .filter((block) => block.kind === 'task')
    .map((block) => {
      const times = block.times ?? {};
      const written = ms(block.version.act.at);
      const from = times.listed ? ms(times.listed) : written;
      const to = times.listed ? (times.ended ? ms(times.ended) : now) : written;
      return {
        key: block.key,
        id: block.id,
        title: block.version.body.title,
        from,
        activeFrom: times.started ? ms(times.started) : null,
        to,
        done: block.version.body.state === 'done',
        fix: block.version.body.type === 'fix',
      };
    });
  // A run kept with the stage it counted toward belongs to that stage, whichever agent ran it.
  const runs = loft
    .filter((run) => (run.stage ? run.stage === stage.key : run.agent === agent && within(run, span)))
    .map(({ from, to, name, failed }) => ({ from, to, name, failed }));
  // A subagent's run kept with no stage, as an older file keeps it, works for the main agent's.
  const helpers = subagents
    .filter((run) => (run.stage ? run.stage === stage.key : !stage.owner.agent && within(run, span)))
    .map(({ from, to, type }) => ({ from, to, type }));
  const views = (stage.blocks ?? [])
    .filter((block) => block.kind === 'loft' && block.version.body.recorded?.at)
    .map((block) => ({ at: ms(block.version.body.recorded.at), view: block.version.body.view, id: block.id }));
  const asked = waits
    .filter((wait) => wait.agent === agent && within(wait, span))
    .map(({ key, from, to, open, block }) => ({ key, from, to, open, block }));
  const active = tasks
    .filter((task) => task.activeFrom !== null)
    .map((task) => ({ from: task.activeFrom, to: task.to }));
  const parts = split(turns.length ? intersect([span], turns) : [span], { loft: runs, subagents: helpers, tasks: active, waits: asked });
  const segments = Object.entries(parts)
    .flatMap(([kind, spans]) =>
      spans.map((each) => ({
        kind,
        ...each,
        block: kind === 'wait' ? (asked.find((wait) => wait.from <= each.from && wait.to >= each.to)?.block ?? null) : null,
      })),
    )
    .sort((a, b) => a.from - b.from);
  return {
    stage: {
      key: stage.key,
      id: stage.id,
      title: stage.version.body.title,
      status: stage.version.body.status,
      ...span,
    },
    tasks,
    loft: runs,
    subagents: helpers,
    views,
    submissions: asked,
    work: parts.work,
    segments,
    active,
  };
}

/** @param {Span} item @param {Span} span */
function within(item, span) {
  return item.from >= span.from && item.from <= span.to;
}

/**
 * Where the time that was not idle went: to Loft runs, then to the subagents' runs, then to active
 * tasks, then to waits on the person, and the rest to the agents' own work, each with its length
 * and its share. Time is counted once, to the first of these it falls in.
 * @param {Span[]} busy
 * @param {any[]} lanes
 */
function summary(busy, lanes) {
  const rows = lanes.flatMap((lane) => lane.rows);
  const spent = split(busy, {
    loft: rows.flatMap((row) => row.loft),
    subagents: rows.flatMap((row) => row.subagents),
    tasks: rows.flatMap((row) => row.active),
    waits: rows.flatMap((row) => row.submissions),
  });
  const total = measure(busy);
  const parts = [
    { name: 'Loft runs', kind: 'loft', length: measure(spent.loft) },
    { name: 'Subagent runs', kind: 'subagent', length: measure(spent.subagent) },
    { name: 'Active tasks', kind: 'task', length: measure(spent.task) },
    { name: 'Waits on you', kind: 'wait', length: measure(spent.wait) },
    { name: 'Agent work', kind: 'work', length: measure(spent.work) },
  ];
  return {
    total,
    parts: parts.map((part) => ({ ...part, share: total ? part.length / total : 0 })),
  };
}

/** The width a folded idle stretch is drawn, in pixels. */
export const FOLD = 28;

/**
 * The idle stretches of `chart` that are folded, in order.
 * @param {{ idle: (Span & { folded: boolean })[] }} chart
 */
export function folds(chart) {
  return chart.idle.filter((span) => span.folded);
}

/**
 * Where each time lies along a chart whatever its width: as a share of the width and an offset in
 * pixels, so that it lies at `share × width + offset` on a chart `width` wide. The time that is
 * not folded divides the width left after each folded idle stretch takes `fold` pixels.
 * @param {{ start: number, end: number, idle: (Span & { folded: boolean })[] }} chart
 * @param {number} [fold]
 * @returns {(time: number) => { share: number, offset: number }}
 */
export function place(chart, fold = FOLD) {
  const folded = folds(chart);
  const hidden = folded.reduce((sum, span) => sum + span.to - span.from, 0);
  const open = Math.max(1, chart.end - chart.start - hidden);
  const at = (before, folds) => {
    const share = before / open;
    return { share, offset: (folds - share * folded.length) * fold };
  };
  return (time) => {
    let before = 0;
    let folds = 0;
    let from = chart.start;
    for (const span of folded) {
      if (time <= span.from) break;
      before += span.from - from;
      if (time < span.to) return at(before, folds + (time - span.from) / (span.to - span.from));
      folds += 1;
      from = span.to;
    }
    return at(before + Math.max(0, Math.min(time, chart.end) - from), folds);
  };
}

/**
 * The x of each time on a chart `width` wide, each folded idle stretch drawn `fold` wide.
 * @param {{ start: number, end: number, idle: (Span & { folded: boolean })[] }} chart
 * @param {number} width
 * @param {number} [fold]
 * @returns {(time: number) => number}
 */
export function scale(chart, width, fold = FOLD) {
  const placed = place(chart, fold);
  return (time) => {
    const { share, offset } = placed(time);
    return share * width + offset;
  };
}

/**
 * The narrowest a chart may be drawn: `perMinute` pixels for each minute that is not folded, and
 * `fold` pixels for each folded idle stretch.
 * @param {{ start: number, end: number, idle: (Span & { folded: boolean })[] }} chart
 * @param {number} [fold]
 * @param {number} [perMinute]
 */
export function minimumWidth(chart, fold = FOLD, perMinute = 8) {
  const folded = folds(chart);
  const hidden = folded.reduce((sum, span) => sum + span.to - span.from, 0);
  return Math.ceil(((chart.end - chart.start - hidden) / 60_000) * perMinute + folded.length * fold);
}

/** The steps the time axis is marked at, in milliseconds: from a minute to an hour. */
const STEPS = [1, 2, 5, 10, 15, 30, 60].map((minutes) => minutes * 60_000);

/**
 * The times the axis is marked at: the chart's start and end, the start of each stretch after a
 * folded idle stretch, and within each stretch the whole multiples of the smallest step that
 * leaves `gap` pixels between marks, short of a fold that ends it, each kept only where it leaves
 * `gap` pixels after the mark before it.
 * @param {{ start: number, end: number, idle: (Span & { folded: boolean })[] }} chart
 * @param {(time: number) => number} x
 * @param {number} [gap]
 * @returns {number[]}
 */
export function ticks(chart, x, gap = 72) {
  /** @type {Span[]} */
  const stretches = [];
  let from = chart.start;
  for (const fold of folds(chart)) {
    stretches.push({ from, to: fold.from });
    from = fold.to;
  }
  stretches.push({ from, to: chart.end });
  const times = [chart.start, ...stretches.slice(1).map((stretch) => stretch.from)];
  for (const [index, stretch] of stretches.entries()) {
    const last = index === stretches.length - 1;
    const pixels = (x(stretch.to) - x(stretch.from)) / Math.max(1, stretch.to - stretch.from);
    const wanted = gap / Math.max(pixels, Number.MIN_VALUE);
    const step = STEPS.find((each) => each >= wanted) ?? Math.ceil(wanted / STEPS[STEPS.length - 1]) * STEPS[STEPS.length - 1];
    for (let time = Math.ceil(stretch.from / step) * step; last ? time <= stretch.to : time < stretch.to; time += step) times.push(time);
  }
  const kept = [];
  for (const time of times.sort((a, b) => a - b)) {
    const last = kept[kept.length - 1];
    if (last === undefined || x(time) - x(last) >= gap) kept.push(time);
  }
  const last = kept[kept.length - 1];
  if (last !== chart.end) {
    if (kept.length > 1 && x(chart.end) - x(kept[kept.length - 2]) >= gap && x(chart.end) - x(last) < gap) kept[kept.length - 1] = chart.end;
    else if (x(chart.end) - x(last) >= gap) kept.push(chart.end);
  }
  return kept;
}
