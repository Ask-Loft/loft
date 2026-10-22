// Each block drawn by its kind, from its current version: a note, a question with the decisions
// that settle it drawn at its head, an approval or a plan with its outcome, a task, a decision
// whose question is not in Yard Log, and a `loft` view. Each card offers who wrote it, its
// version and when, and its earlier versions.

import { h, html } from '../lib/dom.js';
import { when, who } from '../lib/format.js';
import { loftView } from './loft/index.js';

/**
 * @typedef {import('../model/log.js').Block} Block
 * @typedef {{ blocks: Map<string, Block>, decisions: Map<string, Block[]>, marked: Set<string>,
 *   waiting: Set<string>, onAnswer?: (block: Block) => void, branch?: string }} Context
 */

const KINDS = {
  note: 'Note',
  question: 'Question',
  approval: 'Approval',
  plan: 'Plan',
  decision: 'Decision',
  task: 'Task',
  loft: 'Loft',
};

const CHANNELS = { page: 'on the page', terminal: 'in the terminal' };

/**
 * Who acted, when and where.
 * @param {any} response
 */
export function byline(response) {
  const channel = CHANNELS[response.channel] ?? response.channel ?? '';
  return h(
    'span',
    { class: 'byline' },
    h('span', { class: 'who' }, who(response.act?.by)),
    ' ',
    h('time', { datetime: response.act?.at }, when(response.act?.at)),
    channel ? ` ${channel}` : null,
  );
}

/**
 * The card of a block.
 * @param {Block} block
 * @param {Context} context
 */
export function card(block, context) {
  const body = block.version.body;
  const marked = context.marked.has(block.key);
  const waiting = context.waiting.has(block.key);
  const classes = [
    'block',
    `block--${block.kind}`,
    body.tone ? `tone--${body.tone}` : null,
    marked ? 'is-new' : null,
    waiting ? 'is-waiting' : null,
  ];
  return h(
    'article',
    { class: classes, id: `block-${block.id}`, dataset: { key: block.key, kind: block.kind } },
    content(block, context),
    marked ? h('span', { class: 'new-mark' }, 'New since your last visit') : null,
    meta(block),
  );
}

/**
 * @param {Block} block
 * @param {Context} context
 */
function content(block, context) {
  switch (block.kind) {
    case 'note':
      return note(block);
    case 'question':
      return question(block, context);
    case 'approval':
    case 'plan':
      return decidable(block, context);
    case 'decision':
      return decision(block);
    case 'task':
      return task(block);
    case 'loft':
      return loft(block, context);
    default:
      return h('p', {}, `A ${block.kind} block, which this page does not draw.`);
  }
}

/** @param {Block} block */
function note(block) {
  const body = block.version.body;
  const tone = body.tone ? h('span', { class: ['tone-word', `tone--${body.tone}`] }, body.tone) : null;
  const text = html(body.html ?? '', 'prose');
  if (body.folded) {
    return h(
      'details',
      { class: 'fold' },
      h('summary', {}, h('span', { class: 'kind' }, KINDS.note), ' ', body.title ?? 'A folded note', ' ', tone),
      text,
    );
  }
  return [
    h('header', { class: 'block-head' }, h('span', { class: 'kind' }, KINDS.note), body.title ? h('h4', {}, body.title) : null, tone),
    text,
  ];
}

/**
 * The person's answer to a question, in words.
 * @param {any} answer
 */
export function answerText(answer) {
  const parts = [...(answer.labels ?? [])];
  if (answer.words) parts.push(`“${answer.words}”`);
  return parts.join(', ') || 'no option';
}

/**
 * @param {Block} block
 * @param {Context} context
 */
function question(block, context) {
  const body = block.version.body;
  const decisions = context.decisions.get(block.key) ?? [];
  const answer = block.answer;
  const chosen = new Set(answer?.labels ?? []);
  const options = h(
    'ul',
    { class: 'options' },
    body.options.map((option) =>
      h(
        'li',
        { class: ['option', chosen.has(option.label) ? 'is-chosen' : null] },
        h('span', { class: 'option-label' }, option.label),
        option.recommended ? h('span', { class: 'badge badge--recommended' }, 'Recommended') : null,
        chosen.has(option.label) ? h('span', { class: 'badge badge--chosen' }, 'Chosen') : null,
        option.detail ? h('p', { class: 'option-detail' }, option.detail) : null,
      ),
    ),
  );
  const context_ = body.contextHtml ? html(body.contextHtml, 'prose context') : null;
  const prompt = h('p', { class: 'prompt' }, body.prompt);
  const head = decisions.length
    ? h(
        'header',
        { class: 'decision-head' },
        decisions.map((decision) =>
          h(
            'div',
            { class: 'decision-line', id: `block-${decision.id}`, dataset: { key: decision.key } },
            h('span', { class: 'decision-number' }, `Decision ${decision.number ?? ''}`.trim()),
            h('h4', {}, decision.version.body.title),
            h('p', { class: 'consequence' }, decision.version.body.consequence),
          ),
        ),
      )
    : h('header', { class: 'block-head' }, h('span', { class: 'kind' }, KINDS.question), h('h4', {}, body.header));
  if (answer) {
    const answered = h(
      'p',
      { class: 'answer' },
      h('span', { class: 'answer-text' }, answerText(answer)),
      ' ',
      byline(answer),
      answer.notes ? h('span', { class: 'answer-notes' }, ` ${answer.notes}`) : null,
    );
    return [
      head,
      decisions.length ? prompt : null,
      answered,
      h('details', { class: 'fold' }, h('summary', {}, decisions.length ? 'Context and options' : body.prompt), context_, options),
    ];
  }
  const waiting = context.waiting.has(block.key);
  return [
    head,
    prompt,
    context_,
    options,
    waiting
      ? h(
          'p',
          { class: 'waits' },
          'This waits on you. ',
          context.onAnswer
            ? h('button', { type: 'button', class: 'link', onclick: () => context.onAnswer?.(block) }, 'Answer it')
            : null,
        )
      : null,
  ];
}

/**
 * @param {Block} block
 * @param {Context} context
 */
function decidable(block, context) {
  const body = block.version.body;
  const outcome = block.outcome;
  const head = h(
    'header',
    { class: 'block-head' },
    h('span', { class: 'kind' }, KINDS[block.kind]),
    h('h4', {}, body.title),
    outcome ? h('span', { class: ['verdict', `verdict--${outcome.verdict}`] }, outcome.verdict) : null,
  );
  const decided = outcome
    ? h(
        'p',
        { class: 'outcome' },
        h('span', { class: ['verdict-word', `verdict--${outcome.verdict}`] }, outcome.verdict === 'approved' ? 'Approved' : 'Rejected'),
        ' ',
        byline(outcome),
        outcome.reason ? h('span', { class: 'reason' }, `: ${outcome.reason}`) : null,
      )
    : null;
  const waiting = context.waiting.has(block.key);
  return [
    head,
    decided,
    body.path ? h('p', { class: 'path' }, body.path) : null,
    block.kind === 'plan' && body.planMode
      ? h('p', { class: 'plan-mode' }, 'Presented in plan mode: approving it lets the agent go on in auto mode.')
      : null,
    html(body.html ?? '', 'prose'),
    waiting
      ? h(
          'p',
          { class: 'waits' },
          'This waits on you. ',
          context.onAnswer
            ? h('button', { type: 'button', class: 'link', onclick: () => context.onAnswer?.(block) }, 'Decide it')
            : null,
        )
      : null,
  ];
}

/** @param {Block} block */
function decision(block) {
  const body = block.version.body;
  return [
    h('header', { class: 'block-head' }, h('span', { class: 'decision-number' }, `Decision ${block.number ?? ''}`.trim()), h('h4', {}, body.title)),
    h('p', { class: 'consequence' }, body.consequence),
  ];
}

const TASK_STATES = { open: 'Open', active: 'Active', done: 'Done', dropped: 'Dropped' };

/** @param {Block} block */
function task(block) {
  const body = block.version.body;
  return [
    h(
      'header',
      { class: 'block-head' },
      h('span', { class: ['state', `state--${body.state}`] }, TASK_STATES[body.state] ?? body.state),
      h('h4', {}, body.title),
    ),
    body.detail ? h('p', { class: 'detail' }, body.detail) : null,
  ];
}

/**
 * @param {Block} block
 * @param {Context} context
 */
function loft(block, context) {
  const body = block.version.body;
  return [
    h('header', { class: 'block-head' }, h('span', { class: 'kind' }, KINDS.loft), h('h4', {}, VIEW_NAMES[body.view] ?? body.view)),
    body.note ? h('p', { class: 'note' }, body.note) : null,
    loftView(body, { branch: context.branch }),
  ];
}

const VIEW_NAMES = {
  'spec-change': 'Spec change',
  findings: 'Check findings',
  trace: 'Trace coverage',
  decision: 'Decision',
  publish: 'Publish state',
};

/**
 * Who wrote the block, its version and when, and its earlier versions.
 * @param {Block} block
 */
function meta(block) {
  const version = block.version;
  const earlier = (block.versions ?? []).filter((each) => each.key !== version.key).reverse();
  return h(
    'footer',
    { class: 'block-meta' },
    h('span', { class: 'who' }, who(version.act.by)),
    h('span', {}, `version ${version.number}`),
    h('time', { datetime: version.act.at }, when(version.act.at)),
    earlier.length
      ? h(
          'details',
          { class: 'versions' },
          h('summary', {}, earlier.length === 1 ? '1 earlier version' : `${earlier.length} earlier versions`),
          h(
            'ol',
            {},
            earlier.map((each) =>
              h(
                'li',
                {},
                h('span', { class: 'version-head' }, `Version ${each.number}, by ${who(each.act.by)}, ${when(each.act.at)}`),
                earlierBody(each.body),
              ),
            ),
          ),
        )
      : null,
  );
}

/** @param {any} body */
function earlierBody(body) {
  const title = body.title ?? body.prompt ?? body.header;
  return [
    title ? h('strong', {}, title) : null,
    body.html ? html(body.html, 'prose') : null,
    body.goal ? h('p', {}, body.goal) : null,
  ];
}
