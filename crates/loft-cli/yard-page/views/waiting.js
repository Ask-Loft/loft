// What waits on the person in the branch's Yard Log shown: every open submission, oldest first,
// together in the page's modal. It opens by itself for a submission the person has not seen in
// this visit, stays closed once they close it until another is sent, opens again at their asking
// and closes by itself once nothing in it waits.

import { count } from '../lib/format.js';
import * as modal from '../lib/modal.js';
import { answerable, blocksByKey, openSubmissions, waitingBlocks } from '../model/log.js';
import { submissionPanel } from './answering.js';

const NAME = 'waiting';

/**
 * @typedef {import('../model/log.js').Submission} Submission
 * @typedef {import('../model/log.js').Block} Block
 * @typedef {import('./answering.js').Where} Where
 * @typedef {{ submission: Submission, waiting: Block[] }} Waiting
 */

/** The submissions shown in this visit, for each Yard Log by its key. @type {Map<string, Set<string>>} */
const seen = new Map();
/** What the modal shows now, and what it was last drawn from. */
const current = { /** @type {Waiting[]} */ items: [], /** @type {Where | null} */ where: null, drawn: '' };

/**
 * The open submissions of `log` that the person can answer, each with the blocks that still wait.
 * @param {any} log
 * @returns {Waiting[]}
 */
export function waitingIn(log) {
  if (!log || !answerable(log)) return [];
  const blocks = blocksByKey(log);
  return openSubmissions(log)
    .map((submission) => ({ submission, waiting: waitingBlocks(submission, blocks) }))
    .filter((item) => item.waiting.length);
}

/** @param {Waiting[]} items */
function signature(items) {
  return JSON.stringify(items.map((item) => [item.submission.key, item.waiting.map((block) => [block.key, block.version.number, block.versionCount])]));
}

/** The modal's title and the panels of what waits. */
function shown() {
  const where = /** @type {Where} */ (current.where);
  // A panel asks for a refresh where what it shows changed in this browser alone, as when the
  // person accepts the recommendations: the modal draws it at once, and the page loads again.
  const inside = { ...where, refresh: () => { redraw(); where.refresh(); } };
  const blocks = current.items.reduce((sum, item) => sum + item.waiting.length, 0);
  return {
    title: `${count(blocks, 'block')} ${blocks === 1 ? 'waits' : 'wait'} on you`,
    content: current.items.map((item) => submissionPanel(item.submission, item.waiting, inside)),
  };
}

function redraw() {
  if (!current.items.length || !current.where) return;
  current.drawn = signature(current.items);
  modal.update(NAME, shown());
}

/**
 * Brings the modal in line with `log`, the branch's Yard Log just drawn, or with none: it opens
 * for a submission not seen in this visit, is drawn again where what waits changed, and closes
 * once nothing waits.
 * @param {any} log
 * @param {Where} where
 */
export function waitingModal(log, where) {
  const items = waitingIn(log);
  current.items = items;
  current.where = where;
  if (!items.length) {
    current.drawn = '';
    modal.close(NAME);
    return;
  }
  const key = log.yardLog?.key ?? `${where.tree}\n${where.branch}`;
  const known = seen.get(key) ?? new Set();
  seen.set(key, known);
  const fresh = items.some((item) => !known.has(item.submission.key));
  for (const item of items) known.add(item.submission.key);
  if (fresh) {
    openWaiting();
  } else if (modal.isOpen(NAME) && signature(items) !== current.drawn) {
    redraw();
  }
}

/**
 * Opens the modal of what waits, at the form of the block `blockKey` where one is named.
 * @param {string} [blockKey]
 */
export function openWaiting(blockKey) {
  if (!current.items.length || !current.where) return;
  current.drawn = signature(current.items);
  modal.show(NAME, shown());
  if (!blockKey) return;
  const form = modal.body(NAME)?.querySelector(`.question-form[data-key="${blockKey}"], .decision-form[data-key="${blockKey}"]`);
  if (!form) return;
  form.scrollIntoView?.({ block: 'start' });
  /** @type {HTMLElement | null} */ (form.querySelector('input, textarea, button'))?.focus();
}
