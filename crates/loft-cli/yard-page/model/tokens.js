// The tokens Yard Log took, summed over every session's totals file: in all, by stage and within
// each stage by type of subagent, by session, by agent and model, and by submission.

/**
 * @typedef {{ input?: number, cacheRead?: number, cacheWrite?: number, output?: number, partial?: string[] }} Counts
 */

/** @returns {Counts} */
export function zero() {
  return { input: 0, cacheRead: 0, cacheWrite: 0, output: 0 };
}

/** The normalized counters, in display order. */
export const COUNTERS = ['input', 'cacheRead', 'cacheWrite', 'output'];

/**
 * Adds recorded values, preserving absent and partially reported counters.
 * @param {Counts | undefined} into
 * @param {Counts | undefined} counts
 * @returns {Counts}
 */
export function add(into, counts) {
  const result = {};
  const partial = new Set([...(into?.partial ?? []), ...(counts?.partial ?? [])]);
  for (const key of COUNTERS) {
    const left = into?.[key];
    const right = counts?.[key];
    if (left !== undefined || right !== undefined) {
      result[key] = (left ?? 0) + (right ?? 0);
      if (into !== undefined && (left === undefined || right === undefined)) partial.add(key);
    }
  }
  if (partial.size) result.partial = [...partial];
  return result;
}

/** @param {Counts} [counts] */
export function total(counts = {}) {
  return COUNTERS.reduce((sum, key) => sum + (counts[key] ?? 0), 0);
}

/**
 * @template K
 * @param {Map<K, Counts>} map
 * @param {K} key
 * @param {Partial<Counts>} counts
 */
function addTo(map, key, counts) {
  map.set(key, add(map.get(key), counts));
}

/**
 * Yard Log's tokens, from the totals files of its sessions.
 * @param {any[]} files
 * @param {any[]} [activities]
 */
export function summarize(files, activities = []) {
  /** @type {Counts | undefined} */
  let all;
  /** @type {Counts | undefined} */
  let noStage;
  /** @type {Map<string, Counts>} */
  const stages = new Map();
  /** @type {Map<string, Map<string, Counts>>} */
  const stageAgents = new Map();
  /** @type {Map<string, Counts>} */
  const submissions = new Map();
  /** @type {Map<string, { who: string, model: string, tokens: Counts }>} */
  const agents = new Map();
  const sessions = [];
  for (const file of files ?? []) {
    all = add(all, file.tokens);
    noStage = add(noStage, file.noStage ?? Object.fromEntries(COUNTERS.filter((key) => file.tokens?.[key] !== undefined).map((key) => [key, 0])));
    for (const stage of file.stages ?? []) {
      addTo(stages, stage.stage, stage);
      for (const agent of stage.agents ?? []) {
        if (agent.agent === 'main') continue;
        const within = stageAgents.get(stage.stage) ?? new Map();
        addTo(within, agent.type ?? 'subagent', agent);
        stageAgents.set(stage.stage, within);
      }
    }
    for (const submission of file.submissions ?? []) {
      addTo(submissions, submission.submission, submission);
    }
    for (const agent of file.agents ?? []) {
      const who = agent.agent === 'main' ? 'main agent' : agent.type ?? 'subagent';
      const model = agent.model ?? 'unknown';
      const host = file.host ?? 'claude-code';
      const key = `${host}\u0000${who}\u0000${model}`;
      const label = host === 'claude-code' ? who : `${host} ${who}`;
      const held = agents.get(key);
      const named = held ?? { who: label, model };
      agents.set(key, { ...named, tokens: add(held?.tokens, agent) });
    }
    sessions.push({
      host: file.host ?? 'claude-code',
      session: file.session,
      firstEnded: file.firstEnded,
      lastEnded: file.lastEnded,
      tokens: add(undefined, file.tokens),
    });
  }
  for (const activity of activities) {
    const host = activity.host ?? 'claude-code';
    if (!sessions.some((session) => session.host === host && session.session === activity.session)) all = add(all, {});
  }
  sessions.sort((a, b) => String(a.firstEnded).localeCompare(String(b.firstEnded)));
  const byAgent = [...agents.values()].sort(
    (a, b) => Number(b.who === 'main agent') - Number(a.who === 'main agent') || total(b.tokens) - total(a.tokens),
  );
  const partial = all !== undefined && COUNTERS.some((key) => all?.[key] === undefined || all?.partial?.includes(key));
  return { all: all ?? {}, noStage: noStage ?? {}, stages, stageAgents, submissions, agents: byAgent, sessions, partial, counted: COUNTERS.some((key) => all?.[key] !== undefined) };
}
