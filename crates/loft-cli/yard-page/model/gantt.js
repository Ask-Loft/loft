// Yard Log along time, as the Gantt chart draws it: a lane for each agent that owned a stage,
// each lane holding its stages with their tasks, Loft runs, Loft views, waits on the person and
// the agent's own work between them; the idle time outside the main agent's turns, folded where
// it is long; the stretches of plan and spec mode; and where the time that was not idle went.

import { agentKey, ms, who } from '../lib/format.js';

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
  const turns = union(sessions.flatMap((activity) => activity.turns));
  const waits = submissionSpans(log, now);
  const lanes = laneList(log, now, sessions, turns, waits);
  const drawn = lanes.flatMap((lane) =>
    lane.rows.flatMap((row) => [row.stage, ...row.tasks, ...row.loft, ...row.submissions]),
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
    loft: (activity.loft ?? []).map((run) => ({
      ...span(run),
      agent: run.agent === 'main' ? activity.session : `${activity.session}/${run.agent}`,
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
  const blocks = new Map();
  for (const stage of log?.stages ?? []) {
    for (const block of stage.blocks ?? []) blocks.set(block.key, block);
  }
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
 * first started work.
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
  const loft = sessions.flatMap((activity) => activity.loft);
  const built = [...lanes.values()].map((lane) => {
    const stages = lane.stages.sort((a, b) => ms(a.times.started) - ms(b.times.started));
    const rows = stages.map((stage, index) => {
      const next = stages[index + 1];
      const from = ms(stage.times.started);
      let to = stage.times.ended ? ms(stage.times.ended) : next ? ms(next.times.started) : now;
      if (to < from) to = from;
      return row(stage, { from, to }, agentKey(stage.owner), loft, waits, turns, now);
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
    const session = String(stage.owner.session ?? '');
    if (!starts.has(session)) starts.set(session, ms(stage.times.started));
  }
  return [...starts].map(([session, from], index) => ({ session, number: index + 1, from }));
}

/**
 * Where the busy time went, each moment counted once, to the first it falls in of the Loft
 * runs, the active tasks and the waits on the person, and the rest to the agent's own work.
 * @param {Span[]} busy
 * @param {{ loft: Span[], tasks: Span[], waits: Span[] }} spans
 */
function split(busy, { loft, tasks, waits }) {
  const ran = intersect(loft, busy);
  const active = subtract(intersect(tasks, busy), ran);
  const waited = subtract(intersect(waits, busy), [...ran, ...active]);
  const work = subtract(busy, [...ran, ...active, ...waited]);
  return { task: active, loft: ran, wait: waited, work };
}

/**
 * One stage's row: its bar, its tasks, the Loft runs and views made while it was active, its
 * agent's waits on the person, the agent's own work in the rest of its time, and its bar's
 * time divided between them.
 */
function row(stage, span, agent, loft, waits, turns, now) {
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
      };
    });
  const runs = loft
    .filter((run) => run.agent === agent && (run.stage ? run.stage === stage.key : within(run, span)))
    .map(({ from, to, name, failed }) => ({ from, to, name, failed }));
  const views = (stage.blocks ?? [])
    .filter((block) => block.kind === 'loft' && block.version.body.recorded?.at)
    .map((block) => ({ at: ms(block.version.body.recorded.at), view: block.version.body.view, id: block.id }));
  const asked = waits
    .filter((wait) => wait.agent === agent && within(wait, span))
    .map(({ key, from, to, open, block }) => ({ key, from, to, open, block }));
  const active = tasks
    .filter((task) => task.activeFrom !== null)
    .map((task) => ({ from: task.activeFrom, to: task.to }));
  const parts = split(turns.length ? intersect([span], turns) : [span], { loft: runs, tasks: active, waits: asked });
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
 * Where the time that was not idle went: to Loft runs, then to active tasks, then to waits on
 * the person, and the rest to the agents' own work, each with its length and its share. Time is
 * counted once, to the first of these it falls in.
 * @param {Span[]} busy
 * @param {any[]} lanes
 */
function summary(busy, lanes) {
  const rows = lanes.flatMap((lane) => lane.rows);
  const spent = split(busy, {
    loft: rows.flatMap((row) => row.loft),
    tasks: rows.flatMap((row) => row.active),
    waits: rows.flatMap((row) => row.submissions),
  });
  const total = measure(busy);
  const parts = [
    { name: 'Active tasks', kind: 'task', length: measure(spent.task) },
    { name: 'Loft runs', kind: 'loft', length: measure(spent.loft) },
    { name: 'Waits on you', kind: 'wait', length: measure(spent.wait) },
    { name: 'Agent work', kind: 'work', length: measure(spent.work) },
  ];
  return {
    total,
    parts: parts.map((part) => ({ ...part, share: total ? part.length / total : 0 })),
  };
}

/**
 * The x of each time on a chart `width` wide, each folded idle stretch drawn `fold` wide.
 * @param {{ start: number, end: number, idle: (Span & { folded: boolean })[] }} chart
 * @param {number} width
 * @param {number} [fold]
 * @returns {(time: number) => number}
 */
export function scale(chart, width, fold = 28) {
  const folded = chart.idle.filter((span) => span.folded);
  const hidden = folded.reduce((sum, span) => sum + span.to - span.from, 0);
  const open = Math.max(1, chart.end - chart.start - hidden);
  const room = Math.max(1, width - folded.length * fold);
  return (time) => {
    let x = 0;
    let at = chart.start;
    for (const span of folded) {
      if (time <= span.from) break;
      x += ((span.from - at) / open) * room;
      if (time < span.to) return x + ((time - span.from) / (span.to - span.from)) * fold;
      x += fold;
      at = span.to;
    }
    return x + ((Math.min(time, chart.end) - at) / open) * room;
  };
}

/** The steps the time axis is marked at, in milliseconds: from a minute to an hour. */
const STEPS = [1, 2, 5, 10, 15, 30, 60].map((minutes) => minutes * 60_000);

/**
 * The times the axis is marked at: the chart's start and end, and within each stretch between
 * folded idle stretches the whole multiples of the smallest step that leaves `gap` pixels
 * between marks, each kept only where it leaves `gap` pixels after the mark before it.
 * @param {{ start: number, end: number, idle: (Span & { folded: boolean })[] }} chart
 * @param {(time: number) => number} x
 * @param {number} [gap]
 * @returns {number[]}
 */
export function ticks(chart, x, gap = 72) {
  /** @type {Span[]} */
  const stretches = [];
  let from = chart.start;
  for (const fold of chart.idle.filter((span) => span.folded)) {
    stretches.push({ from, to: fold.from });
    from = fold.to;
  }
  stretches.push({ from, to: chart.end });
  const times = [chart.start];
  for (const stretch of stretches) {
    const pixels = (x(stretch.to) - x(stretch.from)) / Math.max(1, stretch.to - stretch.from);
    const wanted = gap / Math.max(pixels, Number.MIN_VALUE);
    const step = STEPS.find((each) => each >= wanted) ?? Math.ceil(wanted / STEPS[STEPS.length - 1]) * STEPS[STEPS.length - 1];
    for (let time = Math.ceil(stretch.from / step) * step; time <= stretch.to; time += step) times.push(time);
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
