// An open submission as the person answers it: each question's options to choose from, with the
// recommended ones marked and the person's own words where the question allows them; a way to
// accept the recommendations; sending every answer at once, offered only once each question
// has one; and approving or rejecting each approval or plan, a rejection with its reason.
// What the person chose and has not sent is kept in this browser only.

import * as api from '../lib/api.js';
import { h, html, markdown } from '../lib/dom.js';
import { when, who } from '../lib/format.js';
import { getJson, setJson } from '../lib/prefs.js';
import {
  PLAN_APPROVAL,
  acceptRecommendations,
  answerOf,
  choose,
  isAnswered,
  readyToSend,
  say,
  toSend,
  verdictToSend,
} from '../model/answers.js';

/**
 * @typedef {{ tree: string, branch: string, refresh: () => void, notify: (text: string) => void }} Where
 */

/** @param {string} key */
function draftOf(key) {
  return getJson(`draft.${key}`, {});
}

/**
 * @param {string} key
 * @param {object} draft
 */
function keep(key, draft) {
  setJson(`draft.${key}`, Object.keys(draft).length ? draft : null);
}

/**
 * The panel of an open submission and the blocks it waits on.
 * @param {import('../model/log.js').Submission} submission
 * @param {import('../model/log.js').Block[]} waiting
 * @param {Where} where
 */
export function submissionPanel(submission, waiting, where) {
  const questions = waiting.filter((block) => block.kind === 'question');
  const message = h('p', { class: 'send-message', role: 'status', 'aria-live': 'polite' });
  const send = h('button', { type: 'button', class: 'primary', id: `send-${submission.key}` }, 'Send answers');
  const update = () => {
    const draft = draftOf(submission.key);
    const ready = readyToSend(draft, questions);
    send.toggleAttribute('disabled', !ready);
    send.title = ready ? '' : 'Answer each question to send';
  };
  send.addEventListener('click', async () => {
    const draft = draftOf(submission.key);
    if (!readyToSend(draft, questions)) return;
    send.setAttribute('disabled', '');
    message.textContent = 'Sending…';
    const reply = await api.answer(where.tree, where.branch, toSend(draft, questions));
    if (isRecorded(reply)) keep(submission.key, {});
    else update();
    message.textContent = sentWords(reply);
    where.notify(message.textContent);
    where.refresh();
  });
  const recommended = questions.some((block) => block.version.body.options.some((option) => option.recommended));
  const panel = h(
    'section',
    { class: 'submission', id: `submission-${submission.key}`, dataset: { key: submission.key } },
    h(
      'header',
      { class: 'submission-head' },
      h('h3', {}, waiting.length === 1 ? 'Waits on you' : `${waiting.length} wait on you`),
      h('p', { class: 'sent' }, 'Sent by ', h('span', { class: 'who' }, who(submission.agent ?? submission.sent?.by)), ' at ', h('time', { datetime: submission.sent?.at }, when(submission.sent?.at))),
    ),
    waiting.map((block) => block.kind === 'question'
      ? questionForm(block, submission.key, update)
      : decisionForm(block, where)),
    questions.length
      ? h(
          'div',
          { class: 'send-row' },
          recommended
            ? h(
                'button',
                {
                  type: 'button',
                  class: 'secondary',
                  onclick: () => {
                    keep(submission.key, acceptRecommendations(draftOf(submission.key), questions));
                    where.refresh();
                  },
                },
                'Accept the recommendations',
              )
            : null,
          send,
        )
      : null,
    message,
  );
  update();
  return panel;
}

/**
 * Whether the page server recorded a send: what it did not record stays to be sent again.
 * @param {{ status: number, body: any }} reply
 */
function isRecorded(reply) {
  return ['answered', 'answered elsewhere'].includes(reply.body?.outcome);
}

/**
 * What the page says once answers are sent.
 * @param {{ status: number, body: any }} reply
 */
export function sentWords(reply) {
  const body = reply.body ?? {};
  if (body.outcome === 'answered') return 'Sent.';
  if (body.outcome === 'answered elsewhere') {
    const where = (body.elsewhere ?? []).map((block) => `${block.id} ${block.channel === 'terminal' ? 'in the terminal' : 'on the page'}`);
    return `Sent. ${where.length ? `Answered elsewhere first, so the first answer is kept: ${where.join(', ')}.` : ''}`.trim();
  }
  const errors = (body.errors ?? []).map((error) => error.message).join(' ');
  return `Not sent: ${errors || `the page server answered ${reply.status}`}.`;
}

/**
 * @param {import('../model/log.js').Block} block
 * @param {string} key the submission's key
 * @param {() => void} update
 */
function questionForm(block, key, update) {
  const body = block.version.body;
  const draft = draftOf(key);
  const answer = answerOf(draft, block.key);
  const type = body.multiple ? 'checkbox' : 'radio';
  const name = `answer-${block.id}`;
  const fieldset = h(
    'fieldset',
    { class: ['question-form', isAnswered(draft, block) ? 'is-answered' : null], dataset: { key: block.key } },
    h('legend', {}, h('span', { class: 'header' }, body.header), ' ', body.prompt),
    body.contextHtml ? html(body.contextHtml, 'prose context') : null,
    body.multiple ? h('p', { class: 'hint' }, 'Choose one or more.') : null,
    body.options.map((option, index) =>
      h(
        'label',
        { class: 'choice', for: `${name}-${index}` },
        h('input', {
          type,
          name,
          id: `${name}-${index}`,
          value: option.label,
          checked: answer.labels.includes(option.label),
          onchange: () => {
            keep(key, choose(draftOf(key), block, option.label));
            fieldset.classList.toggle('is-answered', isAnswered(draftOf(key), block));
            if (!body.multiple) {
              for (const input of fieldset.querySelectorAll(`input[name="${name}"]`)) {
                /** @type {HTMLInputElement} */ (input).checked = /** @type {HTMLInputElement} */ (input).value === option.label;
              }
            }
            update();
          },
        }),
        h('span', { class: 'choice-label' }, option.label),
        option.recommended ? h('span', { class: 'badge badge--recommended' }, 'Recommended') : null,
        markdown(option.detailHtml, option.detail, 'choice-detail'),
      ),
    ),
    body.ownWords
      ? h(
          'label',
          { class: 'words', for: `words-${block.id}` },
          'Or in your own words',
          h('textarea', {
            id: `words-${block.id}`,
            rows: 2,
            oninput: (event) => {
              keep(key, say(draftOf(key), block, event.target.value));
              fieldset.classList.toggle('is-answered', isAnswered(draftOf(key), block));
              update();
            },
          }),
        )
      : null,
  );
  const words = /** @type {HTMLTextAreaElement | null} */ (fieldset.querySelector('textarea'));
  if (words) words.value = answer.words ?? '';
  return fieldset;
}

/**
 * Approving or rejecting an approval or a plan.
 * @param {import('../model/log.js').Block} block
 * @param {Where} where
 */
function decisionForm(block, where) {
  const body = block.version.body;
  // The reason typed and not sent is kept in this browser, as a chosen answer is, so that a
  // change drawn while the person types does not lose it.
  const kept = `reason.${block.key}`;
  const reason = /** @type {HTMLTextAreaElement} */ (
    h('textarea', {
      id: `reason-${block.id}`,
      rows: 2,
      placeholder: 'Why it is rejected',
      oninput: (event) => setJson(kept, event.target.value || null),
    })
  );
  reason.value = getJson(kept, '');
  const message = h('p', { class: 'send-message', role: 'status', 'aria-live': 'polite' });
  const decide = async (verdict) => {
    const sent = verdictToSend(block, verdict, reason.value);
    if (!sent) {
      message.textContent = 'Give the reason for the rejection; it is not sent without one.';
      reason.focus();
      return;
    }
    message.textContent = 'Sending…';
    const reply = await api.answer(where.tree, where.branch, [sent]);
    if (isRecorded(reply)) setJson(kept, null);
    const elsewhere = reply.body?.outcome === 'answered elsewhere';
    message.textContent = elsewhere
      ? 'It was decided elsewhere first, so the first outcome is kept.'
      : sentWords(reply).replace('Sent.', verdict === 'approved' ? 'Approved.' : 'Rejected.');
    where.notify(message.textContent);
    where.refresh();
  };
  return h(
    'div',
    { class: 'decision-form', dataset: { key: block.key } },
    h('h4', {}, h('span', { class: 'kind' }, block.kind === 'plan' ? 'Plan' : 'Approval'), ' ', body.title),
    block.kind === 'plan' ? h('p', { class: 'plan-mode' }, PLAN_APPROVAL) : null,
    html(body.html ?? ''),
    h(
      'div',
      { class: 'decide-row' },
      h('button', { type: 'button', class: 'primary', id: `approve-${block.id}`, onclick: () => decide('approved') }, 'Approve'),
      h('label', { class: 'reason', for: `reason-${block.id}` }, 'Reason for a rejection', reason),
      h('button', { type: 'button', class: 'danger', id: `reject-${block.id}`, onclick: () => decide('rejected') }, 'Reject'),
    ),
    message,
  );
}
