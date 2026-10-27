// The person's answers to a submission while they choose them: kept in this browser only until
// they are sent, and sent all at once.

export const PLAN_APPROVAL = "Approving authorizes the planned work, subject to the agent's mode and permissions.";

/**
 * @typedef {{ labels: string[], words?: string }} Answer
 * @typedef {{ verdict: 'approved' | 'rejected', reason?: string }} Verdict
 * @typedef {Record<string, Answer>} Draft
 */

/**
 * The answer a draft holds for a question, or an empty one.
 * @param {Draft} draft
 * @param {string} key
 * @returns {Answer}
 */
export function answerOf(draft, key) {
  return draft[key] ?? { labels: [] };
}

/**
 * The draft with `label` chosen for the question `block`: the only choice of a question that
 * takes one option, or added to or taken from the choices of one that takes more.
 * @param {Draft} draft
 * @param {any} block
 * @param {string} label
 * @returns {Draft}
 */
export function choose(draft, block, label) {
  const answer = answerOf(draft, block.key);
  let labels;
  if (block.version.body.multiple) {
    labels = answer.labels.includes(label)
      ? answer.labels.filter((chosen) => chosen !== label)
      : [...answer.labels, label];
  } else {
    labels = [label];
  }
  return { ...draft, [block.key]: { ...answer, labels } };
}

/**
 * The draft with the person's own words for the question `block`.
 * @param {Draft} draft
 * @param {any} block
 * @param {string} words
 * @returns {Draft}
 */
export function say(draft, block, words) {
  return { ...draft, [block.key]: { ...answerOf(draft, block.key), words } };
}

/**
 * The draft with the recommended options chosen for each question that has nothing chosen: all
 * of them for a question that takes more than one option, the first for one that takes one.
 * @param {Draft} draft
 * @param {any[]} questions
 * @returns {Draft}
 */
export function acceptRecommendations(draft, questions) {
  let next = { ...draft };
  for (const block of questions) {
    if (block.kind !== 'question' || isAnswered(next, block)) continue;
    const recommended = block.version.body.options
      .filter((option) => option.recommended)
      .map((option) => option.label);
    if (recommended.length === 0) continue;
    const labels = block.version.body.multiple ? recommended : recommended.slice(0, 1);
    next = { ...next, [block.key]: { ...answerOf(next, block.key), labels } };
  }
  return next;
}

/**
 * Whether a draft answers the question `block`: an option chosen, or words given where the
 * question allows them.
 * @param {Draft} draft
 * @param {any} block
 */
export function isAnswered(draft, block) {
  const answer = answerOf(draft, block.key);
  const words = block.version.body.ownWords && (answer.words ?? '').trim() !== '';
  return answer.labels.length > 0 || Boolean(words);
}

/**
 * Whether the questions of a submission can be sent: each of them answered.
 * @param {Draft} draft
 * @param {any[]} questions
 */
export function readyToSend(draft, questions) {
  const asked = questions.filter((block) => block.kind === 'question');
  return asked.length > 0 && asked.every((block) => isAnswered(draft, block));
}

/**
 * The answers to send for `questions`, as the page server takes them.
 * @param {Draft} draft
 * @param {any[]} questions
 */
export function toSend(draft, questions) {
  return questions
    .filter((block) => block.kind === 'question')
    .map((block) => {
      const answer = answerOf(draft, block.key);
      const sent = { block: block.key, labels: answer.labels };
      const words = (answer.words ?? '').trim();
      return words ? { ...sent, words } : sent;
    });
}

/**
 * The decision to send for an approval or a plan: a rejection needs its reason.
 * @param {any} block
 * @param {'approved' | 'rejected'} verdict
 * @param {string} [reason]
 * @returns {{ block: string, verdict: string, reason?: string } | null}
 */
export function verdictToSend(block, verdict, reason) {
  const given = (reason ?? '').trim();
  if (verdict === 'rejected' && !given) return null;
  return given ? { block: block.key, verdict, reason: given } : { block: block.key, verdict };
}
