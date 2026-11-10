// A branch's Yard Log. Its head, as Loft Web heads a page: where it lies, its title and a line
// that sums up its stages and the blocks changed since the last visit, and at its right the
// controls of the view shown, the order of the stages and the count of what waits on the person,
// which opens it in the modal. Beneath the head, whether it is closed, carried from another branch
// or of a newer format, and the view chosen in the tabs along the head's foot: the timeline, the
// Gantt chart, the tokens it took or what its agents consulted.

import { add, h, svg } from '../lib/dom.js';
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
import { consultationView } from './consultation.js';
import { waitingIn } from './waiting.js';

/**
 * @typedef {{ tree: string, branch: string, last: number | null, now: number, width: number,
 *   refresh: () => void, notify: (text: string) => void, onView: () => void,
 *   onPick: (id: string) => void, onWaiting?: (blockKey?: string) => void,
 *   place?: { repository: string, main: boolean },
 *   chosen?: { stage?: string, block?: string },
 *   onChoose?: (stage: string, block?: string) => void }} LogContext
 */

/**
 * What the page shows in place of a Yard Log the page server did not give: that it does not
 * serve the address's tree, where it refused it as such, or else the errors of a Yard Log that
 * does not read.
 * @param {number} status
 * @param {any} body
 * @param {string} tree
 */
export function failedLog(status, body, tree) {
  return status === 404 && body?.errors?.[0]?.kind === 'tree' ? unserved(tree) : unreadable(body);
}

/**
 * That the page server does not serve `tree`, and how to add it.
 * @param {string} tree
 */
function unserved(tree) {
  return h(
    'section',
    { class: 'unreadable', role: 'alert' },
    h('h2', {}, 'The page server does not serve this tree'),
    h('p', {}, h('code', { class: 'tree' }, tree)),
    h('p', {}, 'No session has registered it with the page server yet. A session with the Loft mod, or a loft yard command run in the tree, adds it. To add it now, run on this machine:'),
    h('p', {}, h('code', { class: 'command' }, `loft yard serve --tree ${tree}`)),
  );
}

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
  const answering = answeringIn(log, context);
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
    banners(log),
    view === 'gantt'
      ? gantt(log, { width: context.width, now: context.now, onPick: context.onPick })
      : view === 'tokens'
        ? tokensView(log)
        : view === 'consultation'
          ? consultationView(log)
          : timeline(log, { ...blockContext, order, modes: stretches(log), now: context.now, chosen: context.chosen, onChoose: context.onChoose }),
  );
  return section;
}

const STATUSES = [
  ['done', 'done'],
  ['active', 'active'],
  ['planned', 'planned'],
  ['dropped', 'dropped'],
];

/**
 * The head of the Yard Log shown: where it lies, its title and the line that sums it up, and at
 * its right the controls of the view shown, each labelled above it.
 * @param {any} log
 * @param {LogContext} context
 */
export function logHead(log, context) {
  const yardLog = log.yardLog ?? {};
  const branch = log.branch ?? context.branch;
  const where = context.place;
  const stages = log.stages ?? [];
  const tally = STATUSES.map(([status, word]) => [stages.filter((stage) => stage.version.body.status === status).length, word])
    .filter(([n, word]) => n || word !== 'dropped')
    .map(([n, word]) => `${n} ${word}`);
  const marked = changedSince(log, context.last).size;
  return [
    h(
      'div',
      { class: 'head-text' },
      h(
        'p',
        { class: 'kicker' },
        h('span', { class: 'repository', title: where?.repository ?? context.tree }, base(where?.repository ?? context.tree)),
        where && !where.main ? h('span', { class: 'tree', title: context.tree }, base(context.tree)) : null,
        // The branch is named above the title only where the title is not its name.
        (yardLog.title ?? branch) === branch ? null : h('span', { class: 'branch' }, branch),
      ),
      h('h1', { id: 'log-title' }, yardLog.title ?? branch),
      h(
        'p',
        { class: 'summary' },
        `${count(stages.length, 'stage')}: ${tally.join(', ')}`,
        marked ? [' · ', h('span', { class: 'changed-count' }, `${count(marked, 'block')} changed since your last visit`)] : null,
      ),
    ),
    h('div', { class: 'head-controls' }, orderControl(context), waitControl(log, context)),
    viewTabs(context),
  ];
}

/** @typedef {'timeline' | 'gantt' | 'tokens' | 'consultation'} View */

/** The views of a Yard Log, in their order, each with Loft Web's icon drawn on a square of 16. */
export const VIEWS = /** @type {[View, string, string[]][]} */ ([
  ['timeline', 'Timeline', ['M3.5 3.5h2v2h-2zM3.5 7h2v2h-2zM3.5 10.5h2v2h-2z', 'M8 4.5h5.5M8 8h5.5M8 11.5h5.5']],
  ['gantt', 'Gantt chart', ['M2.5 4h6M5.5 8h7.5M8.5 12h5']],
  ['tokens', 'Tokens', ['M6 2.5l-1.5 11M11.5 2.5l-1.5 11M2.5 6h11M2 10h11']],
  ['consultation', 'Consultation', ['M2.5 13.5h11', 'M4 11V7.5M8 11V3.5M12 11V9']],
]);

/**
 * The tabs of the views along the head's foot, the one shown marked as current. Choosing one
 * keeps the stage and the block chosen, which the address holds.
 * @param {LogContext} context
 */
function viewTabs(context) {
  const shown = prefs.view();
  return h(
    'nav',
    { class: 'view-tabs', 'aria-label': 'Views' },
    VIEWS.map(([view, name, paths]) =>
      h(
        'button',
        {
          type: 'button',
          class: 'view-tab',
          dataset: { view },
          'aria-current': view === shown ? 'page' : null,
          onclick: () => {
            if (view === prefs.view()) return;
            prefs.set('view', view);
            context.onView();
          },
        },
        svg('svg', { class: 'tab-icon', viewBox: '0 0 16 16', width: 16, height: 16, 'aria-hidden': 'true' }, paths.map((d) => svg('path', { d }))),
        name,
      ),
    ),
  );
}

/**
 * The control of the order of the stages, shown only with the timeline.
 * @param {LogContext} context
 */
function orderControl(context) {
  if (prefs.view() !== 'timeline') return null;
  const order = prefs.order();
  const select = h(
    'select',
    {
      id: 'order',
      onchange: (/** @type {Event} */ event) => {
        prefs.set('order', /** @type {HTMLSelectElement} */ (event.target).value);
        context.onView();
      },
    },
    h('option', { value: 'newest', selected: order === 'newest' }, 'Newest first'),
    h('option', { value: 'oldest', selected: order === 'oldest' }, 'Oldest first'),
  );
  return h('label', { class: 'control' }, h('span', { class: 'control-label' }, 'Order of the stages'), select);
}

/**
 * The button that gives the count of what waits on the person and opens it, where it can be
 * answered on the page and anything waits.
 * @param {any} log
 * @param {LogContext} context
 */
function waitControl(log, context) {
  const answering = answeringIn(log, context);
  const waits = waitingIn(log).reduce((sum, item) => sum + item.waiting.length, 0);
  if (!answering || !waits) return null;
  return h(
    'div',
    { class: 'control' },
    h('span', { class: 'control-label', id: 'waiting-label' }, 'Asked of you'),
    h(
      'button',
      { type: 'button', class: 'primary wait-button', id: 'open-waiting', 'aria-describedby': 'waiting-label', onclick: () => answering() },
      `${waits} ${waits === 1 ? 'waits' : 'wait'} on you`,
    ),
  );
}

/**
 * How the person opens what waits on them in `log`, where it can be answered on the page.
 * @param {any} log
 * @param {LogContext} context
 */
function answeringIn(log, context) {
  return context.onWaiting && answerable(log) ? context.onWaiting : undefined;
}

/**
 * Whether the Yard Log shown is closed, carried from another branch or of a newer format.
 * @param {any} log
 */
function banners(log) {
  const yardLog = log.yardLog ?? {};
  const closed = yardLog.closedBy && yardLog.closedBy !== 'open';
  const shown = [
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
  ].filter(Boolean);
  return shown.length ? h('div', { class: 'banners' }, shown) : null;
}
