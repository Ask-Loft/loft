// A branch's Yard Log: its title and where it is, whether it is closed, carried from another
// branch or of a newer format; the count of what waits on the person, which opens it in the
// modal; the blocks changed since the last visit; the timeline or the Gantt chart; and the tokens
// it took.

import { add, h } from '../lib/dom.js';
import { base, count, place, when } from '../lib/format.js';
import * as kept from '../lib/kept.js';
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
import { gantt } from './gantt.js';
import { timeline } from './timeline.js';
import { tokensView } from './tokens.js';
import { waitingIn } from './waiting.js';

/**
 * @typedef {{ tree: string, branch: string, last: number | null, now: number, width: number,
 *   refresh: () => void, notify: (text: string) => void, onView: () => void,
 *   onPick: (id: string) => void, onWaiting?: (blockKey?: string) => void }} LogContext
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
  // What the browser keeps of this Yard Log's folds and scroll is taken up before it is drawn.
  kept.begin(log);
  const blocks = blocksByKey(log);
  const answering = context.onWaiting && answerable(log) ? context.onWaiting : undefined;
  const waitingCount = waitingIn(log).reduce((sum, item) => sum + item.waiting.length, 0);
  const waiting = new Set();
  for (const submission of openSubmissions(log)) {
    for (const block of waitingBlocks(submission, blocks)) waiting.add(block.key);
  }
  const marked = changedSince(log, context.last);
  const order = prefs.order();
  const view = prefs.view();
  const blockContext = {
    blocks,
    decisions: decisionsByQuestion(log),
    marked,
    waiting,
    branch: log.yardLog?.branch,
    onAnswer: answering ? (block) => answering(block.key) : undefined,
  };
  const section = h('section', { class: ['yard-log', `view--${view}`], 'aria-labelledby': 'log-title' });
  add(
    section,
    header(log, context),
    toolbar(order, view, marked.size, context.onView, answering && waitingCount ? { count: waitingCount, open: () => answering() } : null),
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
 * The view at the left, where it stays whichever view is shown, then the order of the stages, the
 * count of what waits on the person, which opens it, and the count of blocks changed since the
 * last visit at the right.
 * @param {'newest' | 'oldest'} order
 * @param {'timeline' | 'gantt'} view
 * @param {number} changed
 * @param {() => void} onView
 * @param {{ count: number, open: () => void } | null} waiting
 */
function toolbar(order, view, changed, onView, waiting) {
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
    waiting
      ? h(
          'button',
          { type: 'button', class: 'primary wait-button', id: 'open-waiting', onclick: waiting.open },
          `${waiting.count} ${waiting.count === 1 ? 'waits' : 'wait'} on you`,
        )
      : null,
    changed ? h('p', { class: 'changed-count' }, `${count(changed, 'block')} changed since your last visit`) : null,
  );
}
