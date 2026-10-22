// The timeline: the stages of a branch's Yard Log in their order, each with its title, its status
// in a word and a colour, its goal and the agent that owns it, the active ones open and the rest
// folded; a line where the stages done or dropped end and the planned ones begin; and in each
// stage its blocks, those written during one stretch of plan or spec mode drawn under a band.

import { h } from '../lib/dom.js';
import { length, ms, when, who } from '../lib/format.js';
import { drawnInQuestion, grouped, ordered } from '../model/log.js';
import { card } from './blocks.js';

const STATUS = { planned: 'Planned', active: 'Active', done: 'Done', dropped: 'Dropped' };

/** Whether the person opened or folded each stage, by its key, for as long as the page is open. */
const folds = new Map();

/**
 * @param {any} log
 * @param {import('./blocks.js').Context & { order: 'newest' | 'oldest', modes: any[], now: number }} context
 */
export function timeline(log, context) {
  const { list, line } = ordered(log.stages ?? [], context.order);
  if (!list.length) return h('p', { class: 'empty' }, 'No stage has been written yet.');
  const items = [];
  list.forEach((stage, index) => {
    if (index === line) {
      const words = context.order === 'oldest' ? 'Planned from here on' : 'Planned above this line';
      items.push(h('li', { class: 'rail-line' }, h('span', {}, words)));
    }
    items.push(h('li', { class: 'station' }, stageSection(stage, context)));
  });
  return h('ol', { class: 'timeline' }, items);
}

/**
 * @param {any} stage
 * @param {import('./blocks.js').Context & { modes: any[], now: number }} context
 */
function stageSection(stage, context) {
  const body = stage.version.body;
  const open = folds.has(stage.key) ? folds.get(stage.key) : body.status === 'active';
  const blocks = (stage.blocks ?? []).filter((block) => !drawnInQuestion(block, context.blocks));
  const owner = stage.owner;
  const times = stage.times ?? {};
  const details = h(
    'details',
    {
      class: ['stage', `status--${body.status}`, context.marked.has(stage.key) ? 'is-new' : null],
      id: `block-${stage.id}`,
      open,
      dataset: { key: stage.key },
      ontoggle: (event) => folds.set(stage.key, /** @type {HTMLDetailsElement} */ (event.currentTarget).open),
    },
    h(
      'summary',
      { class: 'stage-head' },
      h('span', { class: ['status', `status--${body.status}`] }, STATUS[body.status] ?? body.status),
      h('h3', { class: 'stage-title' }, body.title),
      owner ? h('span', { class: 'owner' }, owner.agent ? `${who(owner)} subagent` : 'main agent') : null,
      h('span', { class: 'count' }, blocks.length === 1 ? '1 block' : `${blocks.length} blocks`),
    ),
    body.goal ? h('p', { class: 'goal' }, body.goal) : null,
    times.started
      ? h(
          'p',
          { class: 'stage-times' },
          'Started ',
          h('time', { datetime: times.started }, when(times.started)),
          times.ended ? [', ended ', h('time', { datetime: times.ended }, when(times.ended))] : null,
        )
      : null,
    h(
      'div',
      { class: 'blocks' },
      grouped(blocks, context.modes, context.now).map((item) => ('stretch' in item ? stretchBand(item, context) : card(item.block, context))),
    ),
  );
  return details;
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
