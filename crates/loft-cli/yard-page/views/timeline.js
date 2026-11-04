// The timeline, as Loft Web lists what a page holds beside a pane of the one chosen: at its left,
// the stages of a branch's Yard Log in their order on a rail, each with its title, its status in a
// word and a colour, the agent that owns it and the counts of its blocks that wait on the person
// and that changed since the last visit, a line where the stages done or dropped end and the
// planned ones begin, and each stage open to its blocks or folded to its row; while Yard Log holds
// a single stage, its blocks in place of it, each drawn as a stage's row is. At its right, the
// stage chosen, with its goal and each of its blocks in full, those written during one stretch of
// plan or spec mode drawn under a band. A splitter between them sets the list's width.

import { h, markdown, svg } from '../lib/dom.js';
import { separator } from '../lib/separator.js';
import * as kept from '../lib/kept.js';
import * as prefs from '../lib/prefs.js';
import { count, length, ms, when, who } from '../lib/format.js';
import { drawnInQuestion, grouped, ordered } from '../model/log.js';
import { card } from './blocks.js';

const STATUS = { planned: 'Planned', active: 'Active', done: 'Done', dropped: 'Dropped' };

const KINDS = { note: 'Note', question: 'Question', approval: 'Approval', plan: 'Plan', decision: 'Decision', task: 'Task', loft: 'Loft' };

/** The list's width: its default, its bounds and the step of an arrow key, in pixels. */
export const LIST = { width: 360, least: 240, most: 640, step: 16 };

/**
 * @typedef {import('./blocks.js').Context & {
 *   order: 'newest' | 'oldest', modes: any[], now: number,
 *   chosen?: { stage?: string, block?: string },
 *   onChoose?: (stage: string, block?: string) => void,
 * }} TimelineContext
 */

/**
 * The stage chosen: the one the address names, or the stage of the block it names, else the first
 * `active` stage in the order shown, else the first stage.
 * @param {any[]} list the stages in the order shown
 * @param {{ stage?: string, block?: string }} [chosen]
 */
export function chosenStage(list, chosen = {}) {
  const named = list.find((stage) => stage.id === chosen.stage);
  if (named) return named;
  const holding = chosen.block ? list.find((stage) => (stage.blocks ?? []).some((block) => block.id === chosen.block)) : undefined;
  return holding ?? list.find((stage) => stage.version.body.status === 'active') ?? list[0];
}

/**
 * @param {any} log
 * @param {TimelineContext} context
 */
export function timeline(log, context) {
  const { list, line } = ordered(log.stages ?? [], context.order);
  if (!list.length) return h('p', { class: 'empty' }, 'No stage has been written yet.');
  const chosen = chosenStage(list, context.chosen);
  const items = [];
  if (list.length === 1) items.push(...shownBlocks(chosen, context).map((block) => blockStation(chosen, block, context)));
  else list.forEach((stage, index) => {
    if (index === line) {
      const words = context.order === 'oldest' ? 'Planned from here on' : 'Planned above this line';
      items.push(h('li', { class: 'rail-line' }, h('span', {}, words)));
    }
    items.push(stageRow(stage, stage === chosen, context));
  });
  const split = h('div', { class: 'timeline', style: `--list: ${listWidth()}px` });
  const pane = h('div', { class: 'stage-pane', id: 'stage-pane' }, stagePane(chosen, context));
  const nav = h('nav', { class: 'stage-list', 'aria-label': 'Stages' }, h('ol', { class: 'stages' }, items));
  split.append(nav, splitter(split), pane);
  return split;
}

/** The list's width this browser keeps, within its bounds. */
function listWidth() {
  const kept = Number(prefs.get('split'));
  return Number.isFinite(kept) && kept > 0 ? bounded(kept) : LIST.width;
}

/** @param {number} width */
function bounded(width) {
  return Math.round(Math.min(LIST.most, Math.max(LIST.least, width)));
}

/**
 * The splitter between the list and the pane: dragged, or moved by the arrow keys left and right,
 * it narrows and widens the list, and the browser keeps the width.
 * @param {HTMLElement} split
 */
function splitter(split) {
  let width = listWidth();
  const handle = separator({
    axis: 'x',
    step: LIST.step,
    attributes: {
      class: 'splitter',
      'aria-label': 'Width of the list of stages',
      'aria-controls': 'stage-pane',
      'aria-valuemin': LIST.least,
      'aria-valuemax': LIST.most,
      'aria-valuenow': width,
    },
    read: () => width,
    show: (size) => {
      width = bounded(size);
      split.style.setProperty('--list', `${width}px`);
      handle.setAttribute('aria-valuenow', String(width));
    },
    keep: () => prefs.set('split', String(width)),
  });
  return handle;
}

/** A chevron, pointing right while folded and down while open. */
function chevron() {
  return svg('svg', { class: 'chevron', viewBox: '0 0 16 16', width: 14, height: 14, 'aria-hidden': 'true' }, svg('path', { d: 'M6 4l4 4-4 4' }));
}

/** A stage's blocks, but those drawn in the question they answer. @param {any} stage @param {TimelineContext} context */
function shownBlocks(stage, context) {
  return (stage.blocks ?? []).filter((block) => !drawnInQuestion(block, context.blocks));
}

/**
 * The button of a row of the list, as a stage's row is drawn: its title on its first line and,
 * on its second, its facts.
 * @param {{ title: any, facts: any[], chosen: boolean, onclick: () => void }} row
 */
function rowButton({ title, facts, chosen, onclick }) {
  return h(
    'button',
    { type: 'button', class: 'stage-choose', 'aria-current': chosen ? 'true' : null, onclick },
    h('span', { class: 'stage-title' }, title),
    h('span', { class: 'stage-facts' }, facts),
  );
}

const STATES = { open: 'Open', active: 'Active', done: 'Done', dropped: 'Dropped' };

/**
 * A block's state as a dot and a word, with the status whose mark its row takes on the rail: a
 * task's state, whether a question was answered, an approval's or a plan's verdict, or that it
 * waits on the person. A note, a decision or a report of Loft has none.
 * @param {any} block
 * @param {TimelineContext} context
 * @returns {[string, string, string] | null} the word, its class and the status of its mark
 */
export function blockState(block, context) {
  if (context.waiting.has(block.key)) return ['Waits on you', 'state--waiting', 'active'];
  const body = block.version.body ?? {};
  if (block.kind === 'task') {
    const state = body.state ?? 'open';
    return [STATES[state] ?? state, `state--${state}`, state === 'open' ? 'planned' : state];
  }
  if (block.kind === 'question') return block.answer ? ['Answered', 'state--done', 'done'] : ['Open', 'state--open', 'planned'];
  if (block.kind === 'approval' || block.kind === 'plan') {
    const verdict = block.outcome?.verdict;
    if (verdict === 'approved') return ['Approved', 'state--done', 'done'];
    if (verdict === 'rejected') return ['Rejected', 'state--dropped', 'dropped'];
    return ['Open', 'state--open', 'planned'];
  }
  return null;
}

/**
 * The row of a block of a lone stage, drawn as a stage's row is: its title, then its kind and,
 * where it has one, its state. Choosing it brings the block into view in the pane.
 * @param {any} stage
 * @param {any} block
 * @param {TimelineContext} context
 */
function blockStation(stage, block, context) {
  const state = blockState(block, context);
  const chosen = context.chosen?.block === block.id;
  return h(
    'li',
    {
      class: ['station', 'station--block', `status--${state?.[2] ?? 'note'}`, chosen ? 'is-chosen' : null, context.marked.has(block.key) ? 'is-new' : null],
      id: `row-${block.id}`,
      dataset: { key: block.key, block: block.id },
    },
    h(
      'div',
      { class: 'list-row' },
      rowButton({
        title: blockTitle(block),
        chosen,
        onclick: () => context.onChoose?.(stage.id, block.id),
        facts: [
          h('span', { class: 'kind' }, KINDS[block.kind] ?? block.kind),
          state ? h('span', { class: ['state', state[1]] }, state[0]) : null,
          context.marked.has(block.key) ? h('span', { class: 'changed-mark' }, 'new') : null,
        ],
      }),
    ),
  );
}

/**
 * A stage's row in the list: the button that folds it, the row that chooses it, and while open
 * the rows of its blocks, each of which chooses it and brings the block into view.
 * @param {any} stage
 * @param {boolean} chosen
 * @param {TimelineContext} context
 */
function stageRow(stage, chosen, context) {
  const body = stage.version.body;
  const blocks = shownBlocks(stage, context);
  const owner = stage.owner;
  const path = `${stage.key}/open`;
  // Open while active or chosen and folded otherwise, unless the person chose otherwise under its status.
  const fallback = body.status === 'active' || chosen;
  let open = kept.get(path, fallback, body.status);
  const waits = blocks.filter((block) => context.waiting.has(block.key)).length;
  const marked = blocks.filter((block) => context.marked.has(block.key)).length + (context.marked.has(stage.key) ? 1 : 0);
  const listId = `rows-${stage.id}`;
  const rows = h(
    'ul',
    { class: 'block-rows', id: listId, hidden: !open },
    blocks.map((block) =>
      h(
        'li',
        {},
        h(
          'button',
          {
            type: 'button',
            class: ['block-row', `block-row--${block.kind}`, chosen && context.chosen?.block === block.id ? 'is-chosen' : null, context.waiting.has(block.key) ? 'is-waiting' : null, context.marked.has(block.key) ? 'is-new' : null],
            dataset: { block: block.id },
            onclick: () => context.onChoose?.(stage.id, block.id),
          },
          h('span', { class: 'kind' }, KINDS[block.kind] ?? block.kind),
          h('span', { class: 'block-row-title' }, blockTitle(block)),
        ),
      ),
    ),
  );
  const fold = h(
    'button',
    {
      type: 'button',
      class: 'fold-button',
      'aria-expanded': open ? 'true' : 'false',
      'aria-controls': listId,
      'aria-label': `${open ? 'Fold' : 'Open'} ${body.title}`,
      onclick: (/** @type {Event} */ event) => {
        open = !open;
        kept.set(path, open, fallback, body.status);
        const button = /** @type {HTMLElement} */ (event.currentTarget);
        button.setAttribute('aria-expanded', open ? 'true' : 'false');
        button.setAttribute('aria-label', `${open ? 'Fold' : 'Open'} ${body.title}`);
        rows.hidden = !open;
      },
    },
    chevron(),
  );
  return h(
    'li',
    {
      class: ['station', `status--${body.status}`, chosen ? 'is-chosen' : null, context.marked.has(stage.key) ? 'is-new' : null],
      id: `row-${stage.id}`,
      dataset: { key: stage.key },
    },
    h(
      'div',
      { class: 'list-row' },
      fold,
      rowButton({
        title: body.title,
        chosen,
        onclick: () => context.onChoose?.(stage.id),
        facts: [
          h('span', { class: ['status', `status--${body.status}`] }, STATUS[body.status] ?? body.status),
          owner ? h('span', { class: 'owner' }, owner.agent ? `${who(owner)} subagent` : 'main agent') : null,
          waits ? h('span', { class: 'waiting-count', title: `${waits} wait on you` }, `${waits} waiting`) : null,
          marked ? h('span', { class: 'changed-mark', title: `${marked} changed since your last visit` }, `${marked} new`) : null,
        ],
      }),
    ),
    rows,
  );
}

/**
 * What a block's row in the list calls it, by its kind.
 * @param {any} block
 */
export function blockTitle(block) {
  const body = block.version.body ?? {};
  switch (block.kind) {
    case 'question':
      return body.header || body.prompt || block.id;
    case 'decision':
      return [block.number ? `${block.number}. ` : '', body.title ?? block.id].join('');
    case 'loft':
      return body.view ? `${body.view} view` : block.id;
    case 'note':
      return body.title || firstLine(body.markdown) || block.id;
    default:
      return body.title || block.id;
  }
}

/** @param {string | undefined} text */
function firstLine(text) {
  const line = String(text ?? '').split('\n').find((each) => each.trim()) ?? '';
  return line.replace(/[#*_`>]/g, '').trim().slice(0, 80);
}

/**
 * The stage chosen, in full: its status, the agent that owns it and its count of blocks above its
 * title, its goal and when it started and ended, then each of its blocks.
 * @param {any} stage
 * @param {TimelineContext} context
 */
function stagePane(stage, context) {
  const body = stage.version.body;
  const blocks = shownBlocks(stage, context);
  const owner = stage.owner;
  const times = stage.times ?? {};
  return h(
    'section',
    {
      class: ['stage', `status--${body.status}`, context.marked.has(stage.key) ? 'is-new' : null],
      id: `block-${stage.id}`,
      dataset: { key: stage.key },
      'aria-labelledby': 'stage-title',
    },
    h(
      'header',
      { class: 'stage-head' },
      h(
        'p',
        { class: 'pane-kicker' },
        h('span', { class: ['status', `status--${body.status}`] }, STATUS[body.status] ?? body.status),
        owner ? h('span', { class: 'owner' }, owner.agent ? `${who(owner)} subagent` : 'main agent') : null,
        h('span', { class: 'count' }, count(blocks.length, 'block')),
      ),
      h('h2', { class: 'stage-title', id: 'stage-title' }, body.title),
      markdown(body.goalHtml, body.goal, 'goal'),
      times.started
        ? h(
            'p',
            { class: 'stage-times' },
            'Started ',
            h('time', { datetime: times.started }, when(times.started)),
            times.ended ? [', ended ', h('time', { datetime: times.ended }, when(times.ended))] : null,
          )
        : null,
    ),
    blocks.length
      ? h(
          'div',
          { class: 'blocks' },
          grouped(blocks, context.modes, context.now).map((item) => ('stretch' in item ? stretchBand(item, context) : card(item.block, context))),
        )
      : h('p', { class: 'empty' }, 'No block has been written in this stage yet.'),
  );
}

/**
 * The blocks written during one stretch of plan or spec mode, under a band that names it.
 * @param {{ stretch: any, blocks: any[] }} item
 * @param {import('./blocks.js').Context & { now: number }} context
 */
function stretchBand(item, context) {
  const { stretch } = item;
  const mode = stretch.mode === 'plan' ? 'Plan mode' : 'Spec mode';
  const lasted = stretch.ended
    ? `for ${length(ms(stretch.ended) - ms(stretch.started))}`
    : `and goes on, ${length(context.now - ms(stretch.started))} so far`;
  return h(
    'div',
    { class: ['mode-band', `mode-band--${stretch.mode}`] },
    h('p', { class: 'mode-head' }, h('strong', {}, mode), ` from ${when(stretch.started)} `, lasted),
    item.blocks.map((block) => card(block, context)),
  );
}
