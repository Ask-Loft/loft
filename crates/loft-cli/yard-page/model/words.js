// The words a node had before a change and has after it, compared word by word: the runs the
// two share, and those only the before holds, removed, or only the after holds, added.

/**
 * A run of text and how it fared in the change.
 * @typedef {{ text: string, mark: 'same' | 'added' | 'removed' }} Run
 */

/** The most pairs of words compared one by one; past it, the whole text counts as replaced. */
export const MOST_PAIRS = 250_000;

/**
 * Splits `text` into words, each with the space that follows it, so that the runs joined give
 * the text back.
 * @param {string} text
 */
function words(text) {
  return text.match(/\s*\S+\s*|\s+/g) ?? [];
}

/**
 * Appends `text` to `runs`, joining it to the last run where that has the same mark.
 * @param {Run[]} runs
 * @param {string} text
 * @param {Run['mark']} mark
 */
function push(runs, text, mark) {
  const last = runs.at(-1);
  if (last?.mark === mark) last.text += text;
  else runs.push({ text, mark });
}

/**
 * The runs from `before` to `after`, by the longest sequence of words the two share; words are
 * compared without the space around them.
 * @param {string} before
 * @param {string} after
 * @returns {Run[]}
 */
export function compareWords(before, after) {
  const a = words(before);
  const b = words(after);
  /** @type {Run[]} */
  const runs = [];
  if (a.length * b.length > MOST_PAIRS) {
    if (before) push(runs, before, 'removed');
    if (after) push(runs, after, 'added');
    return runs;
  }
  const bare = (/** @type {string} */ word) => word.trim();
  // shared[i][j]: the longest shared sequence of a[i..] and b[j..].
  const shared = Array.from({ length: a.length + 1 }, () => new Uint32Array(b.length + 1));
  for (let i = a.length - 1; i >= 0; i -= 1) {
    for (let j = b.length - 1; j >= 0; j -= 1) {
      shared[i][j] = bare(a[i]) === bare(b[j]) ? shared[i + 1][j + 1] + 1 : Math.max(shared[i + 1][j], shared[i][j + 1]);
    }
  }
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    if (bare(a[i]) === bare(b[j])) {
      push(runs, b[j], 'same');
      i += 1;
      j += 1;
    } else if (shared[i + 1][j] >= shared[i][j + 1]) {
      push(runs, a[i], 'removed');
      i += 1;
    } else {
      push(runs, b[j], 'added');
      j += 1;
    }
  }
  for (; i < a.length; i += 1) push(runs, a[i], 'removed');
  for (; j < b.length; j += 1) push(runs, b[j], 'added');
  return runs;
}
