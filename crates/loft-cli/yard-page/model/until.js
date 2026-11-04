// How far along time what has no recorded end is drawn: to now while its session is heard, and
// to the session's last event in Yard Log once it has stopped, unheard for a minute; and the time
// the whole chart runs to, which stops at Yard Log's last record once Yard Log is closed, or once
// none of its sessions is heard and no submission is open.

import { agentKey, ms } from '../lib/format.js';

/** The session of an agent, a subagent's included, as the activity names it. @param {any} by */
function sessionOf(by) {
  return by ? agentKey({ host: by.host ?? 'claude-code', session: by.session }) : '';
}

/**
 * The time each session's unended work runs to: its last event, once it stopped; now otherwise.
 * @param {any} log
 * @param {number} now
 * @returns {(by: any) => number} the time for an agent, now for one no activity names
 */
export function caps(log, now) {
  /** @type {Map<string, number>} */
  const stopped = new Map();
  for (const activity of log?.activity ?? []) {
    if (activity.stopped === true && activity.lastEnded) stopped.set(sessionOf(activity), Math.min(now, ms(activity.lastEnded)));
  }
  return (by) => stopped.get(sessionOf(by)) ?? now;
}

/**
 * The time the chart runs to: now while Yard Log is open and one of its sessions is heard or a
 * submission is open; else its last record, so that a closed or quiet Yard Log stops growing.
 * @param {any} log
 * @param {number} now
 */
export function until(log, now) {
  const closed = Boolean(log?.yardLog?.closedBy && log.yardLog.closedBy !== 'open');
  const activity = log?.activity ?? [];
  const heard = !activity.length || activity.some((each) => each.stopped !== true);
  const open = (log?.sent ?? []).some((submission) => submission.state === 'open');
  if (!closed && (heard || open)) return now;
  const last = lastRecord(log);
  return last === null ? now : Math.min(now, last);
}

/**
 * The latest time Yard Log records: of its sessions' activity, its stages and blocks, the
 * versions written and the changes of its submissions.
 * @param {any} log
 * @returns {number | null}
 */
export function lastRecord(log) {
  /** @type {number[]} */
  const times = [];
  /** @param {any} value */
  const add = (value) => {
    const time = typeof value === 'string' ? ms(value) : NaN;
    if (Number.isFinite(time)) times.push(time);
  };
  /** @param {any} span */
  const spanOf = (span) => {
    add(span?.started);
    add(span?.ended);
  };
  for (const activity of log?.activity ?? []) {
    add(activity.lastEnded);
    for (const kind of ['turns', 'plan', 'spec', 'subagents', 'loft']) (activity[kind] ?? []).forEach(spanOf);
  }
  for (const stage of log?.stages ?? []) {
    Object.values(stage.times ?? {}).forEach(add);
    add(stage.version?.act?.at);
    for (const block of stage.blocks ?? []) {
      add(block.version?.act?.at);
      Object.values(block.times ?? {}).forEach(add);
      (block.executions ?? []).forEach(spanOf);
      (block.responses ?? []).forEach((response) => add(response.act?.at));
    }
  }
  for (const submission of log?.sent ?? []) (submission.changes ?? []).forEach((change) => add(change.act?.at));
  return times.length ? Math.max(...times) : null;
}
