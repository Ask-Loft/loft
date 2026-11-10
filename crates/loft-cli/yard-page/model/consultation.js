// What the agents consulted, as the page server sums every session's consultation file in the
// report `loft yard consultation` prints: in all, by stage, by session and by agent type. The page
// sums no counts itself.

/**
 * @typedef {{ specReads: number, codeReads: number, otherReads: number, loftReads: number, specEdits: number, codeEdits: number }} Counts
 * @typedef {{ spec: number, code: number, other: number, unclassified: number }} Estimate
 * @typedef {{ counts: Counts, estimate: Estimate, firstSpec?: string, firstCodeEdit?: string, codeEditsBeforeSpec?: number, specFirst?: boolean }} Scope
 * @typedef {Scope & { stage: string, title?: string }} StageSums
 * @typedef {{ measured: boolean, partial: boolean, all?: Scope, stages: StageSums[], noStage?: Scope,
 *   sessions: { session: string, host: string, counts: Counts, estimate: Estimate, partial: boolean }[],
 *   unmeasured: { session: string, host: string }[],
 *   agents: { type: string, counts: Counts, estimate: Estimate }[] }} ConsultationSums
 */

/** The reads each stage's bar stacks, in its order, the specification's first. */
export const READS = /** @type {const} */ ([
  ['specReads', 'spec reads'],
  ['loftReads', 'loft reads'],
  ['codeReads', 'code reads'],
  ['otherReads', 'other reads'],
]);

/** The classes the estimate shares its tokens among, in its order. */
export const CLASSES = /** @type {const} */ ([
  ['spec', 'spec'],
  ['code', 'code'],
  ['other', 'other'],
  ['unclassified', 'unclassified'],
]);

/** @type {ConsultationSums} */
const NONE = { measured: false, partial: false, stages: [], sessions: [], unmeasured: [], agents: [] };

/**
 * The page server's sums, with each stage's keyed for the views to look up.
 * @param {ConsultationSums | undefined} sums
 */
export function summary(sums = NONE) {
  return { ...sums, byStage: new Map(sums.stages.map((stage) => [stage.stage, stage])) };
}

/** The sum of `counts`' reads and edits. @param {Counts} counts */
export function total(counts) {
  return Object.values(counts).reduce((sum, count) => sum + count, 0);
}

/**
 * The line beside a stage: its consultations of the specification, its reads of the
 * specification and its `loft` reads together, its reads of code, and where its code was edited
 * whether the specification was consulted first or how many edits came before it. None where its
 * agents consulted and edited no file.
 * @param {Scope | undefined} stage
 */
export function line(stage) {
  if (!stage || !total(stage.counts)) return null;
  const consulted = stage.counts.specReads + stage.counts.loftReads;
  const code = stage.counts.codeReads;
  const before = stage.codeEditsBeforeSpec ?? 0;
  const order = stage.specFirst === true ? 'spec-first' : stage.specFirst === false ? 'code-first' : null;
  const edits = `${before} edit${before === 1 ? '' : 's'}`;
  const ordered = order === 'spec-first' ? 'spec first' : order === 'code-first' ? `code first, ${edits}` : null;
  const text = [`spec ${consulted}`, `code ${code}`, ordered].filter(Boolean).join(' · ');
  const said = [
    `${consulted} consultation${consulted === 1 ? '' : 's'} of the specification`,
    `${code} read${code === 1 ? '' : 's'} of code`,
    order === 'spec-first'
      ? 'the specification was consulted before the code was first edited'
      : order === 'code-first'
        ? `${edits} of code came before the specification was consulted`
        : null,
  ]
    .filter(Boolean)
    .join('; ');
  return { consulted, code, order, before, text, said };
}
