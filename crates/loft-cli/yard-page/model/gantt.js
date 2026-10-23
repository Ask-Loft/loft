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
 * The lanes, one for each agent that owned a stage: the main agents first, then the others, in
 * the order each first started work.
 * @param {any} log
 * @param {number} now
 * @param {ReturnType<typeof activities>} sessions
 * @param {Span[]} turns
 * @param {ReturnType<typeof submissionSpans>} waits
 */
function laneList(log, now, sessions, turns, waits) {
  /** @type {Map<string, { key: string, label: string, main: boolean, session: string, stages: any[] }>} */
  const lanes = new Map();
  for (const stage of log?.stages ?? []) {
    const started = stage.times?.started;
    if (!started || !stage.owner) continue;
    const key = agentKey(stage.owner);
    const lane = lanes.get(key) ?? {
      key,
      label: stage.owner.agent ? who(stage.owner) : `main agent, ${stage.owner.session}`,
      main: !stage.owner.agent,
      session: String(stage.owner.session ?? ''),
      stages: [],
    };
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
      return row(stage, { from, to }, lane.key, loft, waits, turns, now);
    });
    return { key: lane.key, label: lane.label, main: lane.main, session: lane.session, rows };
  });
  const first = (lane) => Math.min(...lane.rows.map((row) => row.stage.from));
  return built.sort((a, b) => Number(b.main) - Number(a.main) || first(a) - first(b));
}

/**
 * One stage's row: its bar, its tasks, the Loft runs and views made while it was active, its
 * agent's waits on the person, and the agent's own work in the rest of its time.
 */
function row(stage, span, lane, loft, waits, turns, now) {
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
    .filter((run) => run.agent === lane && (run.stage ? run.stage === stage.key : within(run, span)))
    .map(({ from, to, name, failed }) => ({ from, to, name, failed }));
  const views = (stage.blocks ?? [])
    .filter((block) => block.kind === 'loft' && block.version.body.recorded?.at)
    .map((block) => ({ at: ms(block.version.body.recorded.at), view: block.version.body.view, id: block.id }));
  const asked = waits
    .filter((wait) => wait.agent === lane && within(wait, span))
    .map(({ key, from, to, open, block }) => ({ key, from, to, open, block }));
  const active = tasks
    .filter((task) => task.activeFrom !== null)
    .map((task) => ({ from: task.activeFrom, to: task.to }));
  const busy = [...active, ...runs, ...asked];
  const work = subtract(intersect([span], turns), busy);
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
    work,
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
  const loft = intersect(rows.flatMap((row) => row.loft), busy);
  const tasks = subtract(intersect(rows.flatMap((row) => row.active), busy), loft);
  const waits = subtract(intersect(rows.flatMap((row) => row.submissions), busy), [...loft, ...tasks]);
  const work = subtract(busy, [...waits, ...loft, ...tasks]);
  const total = measure(busy);
  const parts = [
    { name: 'Active tasks', kind: 'task', length: measure(tasks) },
    { name: 'Loft runs', kind: 'loft', length: measure(loft) },
    { name: 'Waits on you', kind: 'wait', length: measure(waits) },
    { name: 'Agent work', kind: 'work', length: measure(work) },
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
