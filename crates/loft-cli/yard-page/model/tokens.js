// The tokens Yard Log took, summed over every session's totals file: in all, by stage, by
// session, by agent and model, and by submission.

/**
 * @typedef {{ input: number, cacheRead: number, cacheWrite: number, output: number }} Counts
 */

/** @returns {Counts} */
export function zero() {
  return { input: 0, cacheRead: 0, cacheWrite: 0, output: 0 };
}

/**
 * `into` with `counts` added.
 * @param {Counts} into
 * @param {Partial<Counts> | undefined} counts
 * @returns {Counts}
 */
export function add(into, counts) {
  return {
    input: into.input + (counts?.input ?? 0),
    cacheRead: into.cacheRead + (counts?.cacheRead ?? 0),
    cacheWrite: into.cacheWrite + (counts?.cacheWrite ?? 0),
    output: into.output + (counts?.output ?? 0),
  };
}

/** @param {Counts} counts */
export function total(counts) {
  return counts.input + counts.cacheRead + counts.cacheWrite + counts.output;
}

/**
 * @template K
 * @param {Map<K, Counts>} map
 * @param {K} key
 * @param {Partial<Counts>} counts
 */
function addTo(map, key, counts) {
  map.set(key, add(map.get(key) ?? zero(), counts));
}

/**
 * Yard Log's tokens, from the totals files of its sessions.
 * @param {any[]} files
 */
export function summarize(files) {
  let all = zero();
  let noStage = zero();
  /** @type {Map<string, Counts>} */
  const stages = new Map();
  /** @type {Map<string, Counts>} */
  const submissions = new Map();
  /** @type {Map<string, { who: string, model: string, tokens: Counts }>} */
  const agents = new Map();
  const sessions = [];
  for (const file of files ?? []) {
    all = add(all, file.tokens);
    noStage = add(noStage, file.noStage);
    for (const stage of file.stages ?? []) addTo(stages, stage.stage, stage);
    for (const submission of file.submissions ?? []) {
      addTo(submissions, submission.submission, submission);
    }
    for (const agent of file.agents ?? []) {
      const who = agent.agent === 'main' ? 'main agent' : agent.type ?? 'subagent';
      const model = agent.model ?? 'unknown';
      const key = `${who}\u0000${model}`;
      const held = agents.get(key) ?? { who, model, tokens: zero() };
      agents.set(key, { ...held, tokens: add(held.tokens, agent) });
    }
    sessions.push({
      session: file.session,
      firstEnded: file.firstEnded,
      lastEnded: file.lastEnded,
      tokens: add(zero(), file.tokens),
    });
  }
  sessions.sort((a, b) => String(a.firstEnded).localeCompare(String(b.firstEnded)));
  const byAgent = [...agents.values()].sort(
    (a, b) => Number(b.who === 'main agent') - Number(a.who === 'main agent') || total(b.tokens) - total(a.tokens),
  );
  return { all, noStage, stages, submissions, agents: byAgent, sessions, counted: total(all) > 0 };
}
