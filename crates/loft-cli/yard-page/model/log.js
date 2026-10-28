// What the page works out from a branch's Yard Log as the page server gives it: the blocks by
// key, the decisions each question is settled by, the open submissions, what waits on the
// person, what changed since the last visit, and the stretches of plan and spec mode.

import { ms } from '../lib/format.js';

/**
 * @typedef {import('../lib/format.js').Act} Act
 * @typedef {import('../lib/format.js').Actor} Actor
 * @typedef {{ key: string, id: string, kind: string, stage?: string, number?: number,
 *   version: { key: string, number: number, act: Act, arrived?: any, body: any },
 *   versions?: any[], versionCount: number, answer?: any, outcome?: any, responses?: any[],
 *   times?: { listed?: string, started?: string, ended?: string }, owner?: Actor,
 *   executions?: { started: string, ended?: string, assignee?: Actor }[],
 *   reached: number, blocks?: Block[] }} Block
 * @typedef {{ key: string, state: string, blocks: string[], sent: Act, sentReached?: number, agent?: Actor }} Submission
 * @typedef {{ mode: 'plan' | 'spec', started: string, ended?: string }} Stretch
 */

/**
 * Every block of every stage, by its key.
 * @param {any} log
 * @returns {Map<string, Block>}
 */
export function blocksByKey(log) {
  /** @type {Map<string, Block>} */
  const found = new Map();
  for (const stage of log?.stages ?? []) {
    for (const block of stage.blocks ?? []) found.set(block.key, block);
  }
  return found;
}

/**
 * The decisions that settle each question, by the question's key, in the order of their
 * numbers.
 * @param {any} log
 * @returns {Map<string, Block[]>}
 */
export function decisionsByQuestion(log) {
  /** @type {Map<string, Block[]>} */
  const found = new Map();
  const blocks = blocksByKey(log);
  for (const block of blocks.values()) {
    const question = block.kind === 'decision' ? block.version.body.question : undefined;
    if (!question || !blocks.has(question)) continue;
    found.set(question, [...(found.get(question) ?? []), block]);
  }
  for (const decisions of found.values()) {
    decisions.sort((a, b) => (a.number ?? 0) - (b.number ?? 0));
  }
  return found;
}

/**
 * Whether a decision is drawn in its question's card rather than as a card of its own.
 * @param {Block} block
 * @param {Map<string, Block>} blocks
 */
export function drawnInQuestion(block, blocks) {
  if (block.kind !== 'decision') return false;
  const question = block.version.body.question;
  return Boolean(question && blocks.get(question)?.kind === 'question');
}

/**
 * Every open submission, each with the agent that sent it, oldest first.
 * @param {any} log
 * @returns {Submission[]}
 */
export function openSubmissions(log) {
  /** @type {Submission[]} */
  const open = [];
  for (const group of log?.submissions ?? []) {
    for (const submission of group.open ?? []) {
      if (submission.state === 'open') open.push({ ...submission, agent: group.agent });
    }
  }
  return open.sort((a, b) => ms(a.sent?.at) - ms(b.sent?.at)
    || (a.sentReached ?? a.sent?.revision ?? 0) - (b.sentReached ?? b.sent?.revision ?? 0)
    || a.key.localeCompare(b.key));
}

/**
 * The blocks of an open submission that still wait on the person.
 * @param {Submission} submission
 * @param {Map<string, Block>} blocks
 * @returns {Block[]}
 */
export function waitingBlocks(submission, blocks) {
  return submission.blocks
    .map((key) => blocks.get(key))
    .filter((block) => block && !decided(block));
}

/**
 * Whether the person answered a question or decided an approval or a plan.
 * @param {Block} block
 */
export function decided(block) {
  return Boolean(block.answer || block.outcome);
}

/**
 * Whether the person can answer or decide in the Yard Log shown: not one carried from another
 * branch, not a closed one, and not one of a newer format.
 * @param {any} log
 */
export function answerable(log) {
  return !log?.inherited && log?.yardLog?.closedBy === 'open' && typeof log?.leftOut !== 'number';
}

/**
 * The keys of the blocks written, answered or decided since the visit at revision `last`, and
 * of each question whose decision was written since. Nothing is marked on a first visit.
 * @param {any} log
 * @param {number | null | undefined} last
 * @returns {Set<string>}
 */
export function changedSince(log, last) {
  /** @type {Set<string>} */
  const marked = new Set();
  if (last === null || last === undefined) return marked;
  const blocks = blocksByKey(log);
  for (const block of blocks.values()) {
    const responded = (block.responses ?? []).some((response) => response.act?.revision > last);
    if (block.reached > last || responded) marked.add(block.key);
  }
  for (const [question, decisions] of decisionsByQuestion(log)) {
    if (decisions.some((decision) => decision.reached > last)) marked.add(question);
  }
  return marked;
}

/**
 * The main agent's stretches of plan mode and of spec mode across every session's activity,
 * in the order they started.
 * @param {any} log
 * @returns {Stretch[]}
 */
export function stretches(log) {
  /** @type {Stretch[]} */
  const found = [];
  for (const activity of log?.activity ?? []) {
    for (const mode of /** @type {const} */ (['plan', 'spec'])) {
      for (const span of activity[mode] ?? []) {
        found.push({ mode, started: span.started, ended: span.ended });
      }
    }
  }
  return found.sort((a, b) => ms(a.started) - ms(b.started));
}

/**
 * The time a block counts as written for a stretch: a task's listing, else its current
 * version's.
 * @param {Block} block
 */
export function writtenAt(block) {
  if (block.kind === 'task' && block.times?.listed) return block.times.listed;
  return block.version.act.at;
}

/**
 * The blocks of a stage in order, those written during one stretch of plan or spec mode drawn
 * together under it. Every plan has a Plan mode band, using its presentation and decision
 * when no native plan stretch covers it. These per-plan bands add no native activity.
 * @param {Block[]} blocks
 * @param {Stretch[]} modes
 * @param {number} now
 * @returns {({ block: Block } | { stretch: Stretch, blocks: Block[] })[]}
 */
export function grouped(blocks, modes, now) {
  /** @type {({ block: Block } | { stretch: Stretch, blocks: Block[] })[]} */
  const items = [];
  for (const block of blocks) {
    const at = ms(writtenAt(block));
    const covers = (mode) => at >= ms(mode.started) && at <= (mode.ended ? ms(mode.ended) : now);
    /** @type {Stretch | undefined} */
    const stretch = block.kind === 'plan'
      ? modes.find((mode) => mode.mode === 'plan' && covers(mode))
        ?? { mode: 'plan', started: writtenAt(block), ended: block.outcome?.act?.at }
      : modes.find(covers);
    const last = items[items.length - 1];
    if (stretch && last && 'stretch' in last && last.stretch === stretch) {
      last.blocks.push(block);
    } else if (stretch) {
      items.push({ stretch, blocks: [block] });
    } else {
      items.push({ block });
    }
  }
  return items;
}

/**
 * The stages in the order chosen, and the index before which the line between the stages done
 * or dropped and the stages planned is drawn, or -1 where no such line is drawn.
 * @param {any[]} stages
 * @param {'newest' | 'oldest'} order
 */
export function ordered(stages, order) {
  const list = order === 'newest' ? [...stages].reverse() : [...stages];
  const planned = (stage) => stage.version.body.status === 'planned';
  let line = -1;
  for (let index = 1; index < list.length; index += 1) {
    const [before, after] = [list[index - 1], list[index]];
    const crossing =
      order === 'oldest' ? !planned(before) && planned(after) : planned(before) && !planned(after);
    if (crossing) {
      line = index;
      break;
    }
  }
  return { list, line };
}
