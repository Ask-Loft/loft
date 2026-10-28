// Yard Log as the page server gives it, built a piece at a time for the page's tests.

let next = 0;

/** A key of its own. */
export function key() {
  next += 1;
  return `00000000-0000-7000-8000-${String(next).padStart(12, '0')}`;
}

export const MAIN = { host: 'claude-code', session: 's1' };
export const EXPLORE = { host: 'claude-code', session: 's1', agent: 'a1', agentType: 'Explore' };
export const PERSON = { name: 'Ada Reader', email: 'ada@example.com' };

/**
 * An Act.
 * @param {string} at
 * @param {number} revision
 * @param {object} [by]
 */
export function act(at, revision, by = MAIN) {
  return { at, revision, by };
}

/**
 * A block of `kind` with `body`, written at `at` in revision `revision`.
 * @param {string} kind
 * @param {string} id
 * @param {object} body
 * @param {{ at?: string, revision?: number, by?: object, more?: object }} [options]
 */
export function block(kind, id, body, { at = '2026-01-01T10:00:00Z', revision = 1, by = MAIN, more = {} } = {}) {
  const version = { key: key(), number: 1, act: act(at, revision, by), body };
  return {
    key: key(),
    id,
    kind,
    version,
    versions: [version],
    versionCount: 1,
    responses: [],
    reached: revision,
    ...more,
  };
}

/**
 * A stage holding `blocks`.
 * @param {string} id
 * @param {string} title
 * @param {string} status
 * @param {object[]} blocks
 * @param {object} [more]
 */
export function stage(id, title, status, blocks, more = {}) {
  const made = block('stage', id, { title, status, goal: `The goal of ${title}` }, { more });
  for (const each of blocks) each.stage = made.key;
  return { ...made, blocks };
}

/**
 * A question with `labels` as its options, the first recommended.
 * @param {string} id
 * @param {string[]} labels
 * @param {object} [more]
 */
export function question(id, labels, more = {}) {
  return block('question', id, {
    prompt: `Which ${id}?`,
    header: id,
    options: labels.map((label, index) => ({ label, recommended: index === 0, detail: `About ${label}` })),
    multiple: false,
    ownWords: false,
    ...more,
  });
}

/**
 * The person's answer, given on `channel` at `at`.
 * @param {string[]} labels
 * @param {string} channel
 * @param {string} [at]
 * @param {number} [revision]
 */
export function answer(labels, channel, at = '2026-01-01T10:05:00Z', revision = 2) {
  return { key: key(), labels, channel, act: act(at, revision, PERSON) };
}

/**
 * A branch's Yard Log of `stages`.
 * @param {object[]} stages
 * @param {object} [more]
 */
export function log(stages, more = {}) {
  return {
    schema: 'loft-yard-read/1',
    outcome: 'read',
    tree: '/work/shelf',
    branch: 'feat/shelf',
    revision: 3,
    inherited: false,
    yardLog: { key: key(), title: 'Shelf', branch: 'feat/shelf', format: 'yardlog/1', closedBy: 'open', merged: [] },
    stages,
    submissions: [],
    activity: [],
    sent: [],
    ...more,
  };
}

/**
 * An open submission of `blocks`, sent at `at` by `agent`.
 * @param {object[]} blocks
 * @param {string} at
 * @param {object} [agent]
 */
export function open(blocks, at, agent = MAIN) {
  return {
    agent,
    open: [{ key: key(), state: 'open', blocks: blocks.map((each) => each.key), sent: act(at, 2, agent) }],
  };
}
