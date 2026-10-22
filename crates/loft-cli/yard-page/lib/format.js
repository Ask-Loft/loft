// Words and numbers as the page shows them: times, lengths of time, counts, and who acted.

/**
 * @typedef {{ host?: string, session?: string, agent?: string, agentType?: string,
 *   name?: string, email?: string }} Actor
 * @typedef {{ at: string, revision: number, by: Actor }} Act
 */

/**
 * Milliseconds since the epoch of an RFC 3339 time, or NaN.
 * @param {string | undefined} at
 */
export function ms(at) {
  return at ? Date.parse(at) : NaN;
}

/**
 * A time as the page shows it: the time of day for today, else the date and the time.
 * @param {string | undefined} at
 * @param {Date} [now]
 */
export function when(at, now = new Date()) {
  const time = ms(at);
  if (Number.isNaN(time)) return '';
  const date = new Date(time);
  const clock = date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  if (date.toDateString() === now.toDateString()) return clock;
  return `${date.toLocaleDateString([], { day: 'numeric', month: 'short' })}, ${clock}`;
}

/**
 * A length of time in words: `45 s`, `12 min`, `2 h 5 min`.
 * @param {number} millis
 */
export function length(millis) {
  const seconds = Math.max(0, Math.round(millis / 1000));
  if (seconds < 60) return `${seconds} s`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest ? `${hours} h ${rest} min` : `${hours} h`;
}

/**
 * A count of tokens: `950`, `12.4k`, `3.1M`.
 * @param {number} count
 */
export function tokens(count) {
  if (!count) return '0';
  if (count < 1000) return String(count);
  if (count < 1_000_000) return `${(count / 1000).toFixed(count < 10_000 ? 1 : 0)}k`;
  return `${(count / 1_000_000).toFixed(1)}M`;
}

/**
 * Who acted, in words: the person's name, `main agent` or the subagent's type.
 * @param {Actor | undefined} by
 */
export function who(by) {
  if (!by) return 'unknown';
  if (by.name) return by.name;
  if (by.agentType) return by.agentType;
  if (by.agent) return 'subagent';
  return 'main agent';
}

/**
 * The agent an actor names, as one key: its session, and its subagent where it is one.
 * @param {Actor | undefined} by
 */
export function agentKey(by) {
  if (!by) return '';
  return by.agent ? `${by.session}/${by.agent}` : String(by.session ?? '');
}

/**
 * A word with its count: `1 block`, `3 blocks`.
 * @param {number} count
 * @param {string} one
 * @param {string} [many]
 */
export function count(count, one, many = `${one}s`) {
  return `${count} ${count === 1 ? one : many}`;
}

/**
 * The last part of a path.
 * @param {string} path
 */
export function base(path) {
  const parts = String(path ?? '').split('/').filter(Boolean);
  return parts[parts.length - 1] ?? path;
}
