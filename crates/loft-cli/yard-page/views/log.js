// A branch's Yard Log: its title and where it is, whether it is closed, carried from another
// branch or of a newer format; what waits on the person; the blocks changed since the last
// visit; the timeline or the Gantt chart; and the tokens it took.

import { add, h } from '../lib/dom.js';
import { base, count, place, when } from '../lib/format.js';
import * as prefs from '../lib/prefs.js';
import {
  answerable,
  blocksByKey,
  changedSince,
  decisionsByQuestion,
  openSubmissions,
  stretches,
  waitingBlocks,
} from '../model/log.js';
import { submissionPanel } from './answering.js';
import { gantt } from './gantt.js';
import { timeline } from './timeline.js';
import { tokensView } from './tokens.js';

/**
 * @typedef {{ tree: string, branch: string, last: number | null, now: number, width: number,
 *   refresh: () => void, notify: (text: string) => void, onView: () => void,
 *   onPick: (id: string) => void }} LogContext
 */

/**
 * The error a Yard Log that does not read gives, in place of its stages.
 * @param {any} body
 */
export function unreadable(body) {
  const errors = body?.errors ?? [];
  return h(
    'section',
    { class: 'unreadable', role: 'alert' },
    h('h2', {}, 'This Yard Log does not read'),
    h('p', {}, 'Its files cannot be read, as after a bad edit by hand or a merge conflict left unresolved. Nothing is written to it until they are fixed.'),
    h(
      'ul',
      {},
      errors.map((error) =>
        h(
          'li',
          {},
          error.message,
          (error.sites ?? []).map((site) => h('code', { class: 'site' }, ` ${place(site)}`)),
        ),
      ),
    ),
  );
}

/**
 * @param {any} log
 * @param {LogContext} context
 */
export function logView(log, context) {
  const blocks = blocksByKey(log);
  const open = answerable(log) ? openSubmissions(log) : [];
  const waiting = new Set();
  for (const submission of openSubmissions(log)) {
    for (const block of waitingBlocks(submission, blocks)) waiting.add(block.key);
  }
  const marked = changedSince(log, context.last);
  const where = { tree: context.tree, branch: context.branch, refresh: context.refresh, notify: context.notify };
  const order = prefs.order();
  const view = prefs.view();
  const blockContext = {
    blocks,
    decisions: decisionsByQuestion(log),
    marked,
    waiting,
    branch: log.yardLog?.branch,
    onAnswer: answerable(log)
      ? (block) => document.querySelector(`[data-key="${block.key}"].question-form, .decision-form[data-key="${block.key}"]`)?.scrollIntoView({ behavior: 'smooth', block: 'center' })
      : undefined,
  };
  const section = h('section', { class: ['yard-log', `view--${view}`], 'aria-labelledby': 'log-title' });
  add(
    section,
    header(log, context),
    open.length
      ? h(
          'div',
          { class: 'waiting', 'aria-label': 'What waits on you here' },
          open.map((submission) => submissionPanel(submission, waitingBlocks(submission, blocks), where)),
        )
      : null,
    toolbar(order, view, marked.size, context.onView),
    view === 'gantt'
      ? gantt(log, { width: context.width, now: context.now, onPick: context.onPick })
      : timeline(log, { ...blockContext, order, modes: stretches(log), now: context.now }),
    tokensView(log),
  );
  return section;
}

/**
 * @param {any} log
 * @param {LogContext} context
 */
function header(log, context) {
  const yardLog = log.yardLog ?? {};
  const closed = yardLog.closedBy && yardLog.closedBy !== 'open';
  return h(
    'header',
    { class: 'log-head' },
    h('p', { class: 'where' }, h('span', { class: 'tree' }, base(context.tree)), h('span', { class: 'branch' }, log.branch ?? context.branch)),
    h('h2', { id: 'log-title' }, yardLog.title ?? log.branch),
    closed
      ? h(
          'p',
          { class: 'banner banner--closed' },
          yardLog.closedBy === 'merge'
            ? 'Closed by its merge into the base branch. Nothing here can be answered or decided.'
            : ['Closed ', h('time', { datetime: yardLog.closed?.at }, when(yardLog.closed?.at)), '. Nothing here can be answered or decided.'],
        )
      : null,
    log.inherited
      ? h('p', { class: 'banner banner--inherited' }, `Carried from another branch into this worktree, which is on another branch. It is shown to read only.`)
      : null,
    typeof log.leftOut === 'number'
      ? h(
          'p',
          { class: 'banner banner--newer' },
          `Written in the format ${yardLog.format}, newer than this loft reads: a newer loft is needed to show it whole and to answer here. ${count(log.leftOut, 'record')} left out.`,
        )
      : null,
  );
}

/**
 * The order of the stages and the view, beside the count of blocks changed since the last visit.
 * @param {'newest' | 'oldest'} order
 * @param {'timeline' | 'gantt'} view
 * @param {number} changed
 * @param {() => void} onView
 */
function toolbar(order, view, changed, onView) {
  const choice = (name, value, label, current) =>
    h(
      'button',
      {
        type: 'button',
        class: ['segment', current === value ? 'is-on' : null],
        'aria-pressed': current === value ? 'true' : 'false',
        onclick: () => {
          prefs.set(name, value);
          onView();
        },
      },
      label,
    );
  return h(
    'div',
    { class: 'toolbar' },
    changed ? h('p', { class: 'changed-count' }, `${count(changed, 'block')} changed since your last visit`) : h('span', {}),
    h(
      'div',
      { class: 'segments', role: 'group', 'aria-label': 'View' },
      choice('view', 'timeline', 'Timeline', view),
      choice('view', 'gantt', 'Gantt chart', view),
    ),
    view === 'timeline'
      ? h(
          'div',
          { class: 'segments', role: 'group', 'aria-label': 'Order of the stages' },
          choice('order', 'newest', 'Newest first', order),
          choice('order', 'oldest', 'Oldest first', order),
        )
      : null,
  );
}
