// The tokens Yard Log took, as the page server sums them over every session's totals file: in
// all, by stage and within each stage by type of subagent, by session, by agent and model, and
// by submission. The page sums no counts itself.

/**
 * @typedef {{ input?: number, cacheRead?: number, cacheWrite?: number, output?: number, partial?: string[] }} Counts
 * @typedef {{ all: Counts, noStage: Counts,
 *   stages: { stage: string, tokens: Counts, agents: { type: string, tokens: Counts }[] }[],
 *   submissions: { submission: string, tokens: Counts }[],
 *   agents: { who: string, model: string, tokens: Counts }[],
 *   sessions: { host: string, session: string, firstEnded?: string, lastEnded?: string, tokens: Counts }[],
 *   partial: boolean, counted: boolean }} TokenSums
 */

/** The counters, in display order. */
export const COUNTERS = ['input', 'cacheRead', 'cacheWrite', 'output'];

/** @type {TokenSums} */
const NONE = { all: {}, noStage: {}, stages: [], submissions: [], agents: [], sessions: [], partial: false, counted: false };

/**
 * The page server's sums of a Yard Log, each breakdown keyed for the view to look up.
 * @param {TokenSums | undefined} sums
 */
export function summary(sums = NONE) {
  return {
    ...sums,
    stages: new Map(sums.stages.map((stage) => [stage.stage, stage])),
    submissions: new Map(sums.submissions.map((submission) => [submission.submission, submission.tokens])),
  };
}

/** Whether any counter of `counts` is more than zero. @param {Counts} counts */
export function any(counts) {
  return COUNTERS.some((key) => (counts[key] ?? 0) > 0);
}
