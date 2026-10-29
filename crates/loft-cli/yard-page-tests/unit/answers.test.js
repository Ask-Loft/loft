// Answering a submission on the page: choosing, accepting the recommendations, sending once each
// question has an answer, and approving or rejecting with a reason.

import { forget, root } from './dom.js';

import assert from 'node:assert/strict';
import { afterEach, beforeEach, test } from 'node:test';

import { acceptRecommendations, choose, readyToSend, say, toSend, verdictToSend } from '../../yard-page/model/answers.js';
import { sentWords, submissionPanel } from '../../yard-page/views/answering.js';
import { block, question } from './fixtures.js';

/** @type {any[]} */
let posted = [];
const fetched = globalThis.fetch;

beforeEach(() => {
  forget();
  posted = [];
  globalThis.fetch = async (path, init) => {
    posted.push({ path, body: JSON.parse(String(init?.body ?? 'null')) });
    return new Response(JSON.stringify({ outcome: 'answered', errors: [] }), { status: 200 });
  };
});

afterEach(() => {
  globalThis.fetch = fetched;
});

const where = { tree: '/work/shelf', branch: 'feat/shelf', refresh: () => {}, notify: () => {} };

test('a question that takes one option lets the person choose one', () => {
  const asked = question('shelf', ['Top', 'Bottom']);
  let draft = choose({}, asked, 'Top');
  draft = choose(draft, asked, 'Bottom');
  assert.deepEqual(draft[asked.key].labels, ['Bottom']);
  const panel = submissionPanel({ key: 's', state: 'open', blocks: [asked.key], sent: { at: '', revision: 1, by: {} } }, [asked], where);
  assert.deepEqual([...panel.querySelectorAll('input:not([id$="-words"])')].map((input) => input.type), ['radio', 'radio']);
});

test('a question that takes more than one option lets the person choose one or more', () => {
  const asked = question('shelves', ['Top', 'Middle', 'Bottom'], { multiple: true });
  let draft = choose({}, asked, 'Top');
  draft = choose(draft, asked, 'Bottom');
  assert.deepEqual(draft[asked.key].labels, ['Top', 'Bottom']);
  draft = choose(draft, asked, 'Top');
  assert.deepEqual(draft[asked.key].labels, ['Bottom']);
  const panel = submissionPanel({ key: 's', state: 'open', blocks: [asked.key], sent: { at: '', revision: 1, by: {} } }, [asked], where);
  assert.equal(panel.querySelectorAll('input[type=checkbox]:not([id$="-words"])').length, 3);
});

test('every question offers its own-words option with a text field after its named options', () => {
  const asked = question('shelf', ['Top', 'Bottom']);
  const panel = submissionPanel({ key: 's', state: 'open', blocks: [asked.key], sent: { at: '', revision: 1, by: {} } }, [asked], where);
  assert.ok(panel.querySelector(`#words-${asked.id}`));
  assert.equal(panel.querySelector(`#answer-${asked.id}-words`).type, 'radio');
  const radios = [...panel.querySelectorAll('input[type=radio]')];
  assert.equal(radios.length, 3);
  assert.equal(radios.at(-1).id, `answer-${asked.id}-words`);
  const draft = say({}, asked, 'The window sill');
  assert.ok(readyToSend(draft, [asked]));
  assert.deepEqual(toSend(draft, [asked]), [{ block: asked.key, labels: [], words: 'The window sill' }]);
});

test('each option shows its detail and each recommended option is marked', () => {
  const asked = question('shelf', ['Top', 'Bottom']);
  const panel = submissionPanel({ key: 's', state: 'open', blocks: [asked.key], sent: { at: '', revision: 1, by: {} } }, [asked], where);
  const choices = [...panel.querySelectorAll('.choice')].filter((choice) => choice.querySelector('.choice-detail'));
  assert.deepEqual(choices.map((choice) => choice.querySelector('.choice-detail').textContent), ['About Top', 'About Bottom']);
  assert.deepEqual(choices.map((choice) => Boolean(choice.querySelector('.badge--recommended'))), [true, false]);
});

test('an option’s detail is drawn from the HTML loft rendered from its Markdown', () => {
  const asked = question('shelf', ['Top', 'Bottom']);
  asked.version.body.options[0].detailHtml = '<p>The <strong>top</strong> one</p>';
  const panel = submissionPanel({ key: 's', state: 'open', blocks: [asked.key], sent: { at: '', revision: 1, by: {} } }, [asked], where);
  const details = [...panel.querySelectorAll('.choice-detail')];
  assert.equal(details[0].querySelector('strong').textContent, 'top');
  assert.equal(details[1].textContent, 'About Bottom');
});

test('accepting the recommendations chooses the recommended options of each question with nothing chosen', () => {
  const one = question('one', ['A', 'B']);
  const many = block('question', 'many', {
    prompt: 'Which?',
    header: 'many',
    options: [{ label: 'X', recommended: true }, { label: 'Y', recommended: true }, { label: 'Z', recommended: false }],
    multiple: true,
    ownWords: false,
  });
  const none = question('none', ['P', 'Q']);
  none.version.body.options = none.version.body.options.map((option) => ({ ...option, recommended: false }));
  const chosen = question('chosen', ['M', 'N']);
  const draft = acceptRecommendations(choose({}, chosen, 'N'), [one, many, none, chosen]);
  assert.deepEqual(draft[one.key].labels, ['A']);
  assert.deepEqual(draft[many.key].labels, ['X', 'Y']);
  assert.equal(draft[none.key], undefined, 'a question with no recommended option is left for the person');
  assert.deepEqual(draft[chosen.key].labels, ['N'], 'the person’s own choice is kept');
});

test('the page offers to send only once each open question of the submission has an answer', () => {
  const first = question('first', ['A', 'B']);
  const second = question('second', ['C', 'D']);
  assert.equal(readyToSend(choose({}, first, 'A'), [first, second]), false);
  assert.equal(readyToSend(choose(choose({}, first, 'A'), second, 'C'), [first, second]), true);
  const panel = submissionPanel({ key: 's2', state: 'open', blocks: [first.key, second.key], sent: { at: '', revision: 1, by: {} } }, [first, second], where);
  assert.ok(panel.querySelector('#send-s2').hasAttribute('disabled'));
});

test('sending sends every answer of the submission at once', async () => {
  const first = question('first', ['A', 'B']);
  const second = question('second', ['C', 'D']);
  localStorage.setItem('yard.draft.s3', JSON.stringify(choose(choose({}, first, 'A'), second, 'D')));
  const panel = submissionPanel({ key: 's3', state: 'open', blocks: [first.key, second.key], sent: { at: '', revision: 1, by: {} } }, [first, second], where);
  root().append(panel);
  const send = panel.querySelector('#send-s3');
  assert.equal(send.hasAttribute('disabled'), false);
  send.click();
  await new Promise((resolve) => setTimeout(resolve, 10));
  assert.equal(posted.length, 1);
  assert.equal(posted[0].path, '/api/answer');
  assert.deepEqual(posted[0].body, {
    tree: '/work/shelf',
    branch: 'feat/shelf',
    answers: [
      { block: first.key, labels: ['A'] },
      { block: second.key, labels: ['D'] },
    ],
  });
  assert.equal(localStorage.getItem('yard.draft.s3'), null, 'nothing is kept once sent');
});

test('answers chosen and not sent are kept in the browser only', () => {
  const asked = question('shelf', ['Top', 'Bottom']);
  const panel = submissionPanel({ key: 's4', state: 'open', blocks: [asked.key], sent: { at: '', revision: 1, by: {} } }, [asked], where);
  root().append(panel);
  const input = panel.querySelector('input[value="Bottom"]');
  input.checked = true;
  input.dispatchEvent(new Event('change'));
  assert.deepEqual(JSON.parse(localStorage.getItem('yard.draft.s4'))[asked.key].labels, ['Bottom']);
  assert.equal(posted.length, 0, 'nothing reaches the page server until it is sent');
  const again = submissionPanel({ key: 's4', state: 'open', blocks: [asked.key], sent: { at: '', revision: 1, by: {} } }, [asked], where);
  assert.equal(again.querySelector('input[value="Bottom"]').checked, true, 'a choice is kept through a change');
});

test('a rejection without a reason is not sent, and the page asks for the reason', async () => {
  const approval = block('approval', 'ship', { title: 'Ship it?', markdown: 'x', html: '<p>x</p>' });
  assert.equal(verdictToSend(approval, 'rejected', '  '), null);
  assert.deepEqual(verdictToSend(approval, 'rejected', 'Too soon'), { block: approval.key, verdict: 'rejected', reason: 'Too soon' });
  assert.deepEqual(verdictToSend(approval, 'approved'), { block: approval.key, verdict: 'approved' });
  const panel = submissionPanel({ key: 's5', state: 'open', blocks: [approval.key], sent: { at: '', revision: 1, by: {} } }, [approval], where);
  root().append(panel);
  panel.querySelector('#reject-ship').click();
  await new Promise((resolve) => setTimeout(resolve, 10));
  assert.equal(posted.length, 0);
  assert.match(panel.querySelector('.decision-form .send-message').textContent, /Give the reason/);
});

test('a reason for a rejection typed and not sent is kept through a change, and forgotten once sent', async () => {
  const approval = block('approval', 'ship', { title: 'Ship it?', markdown: 'x', html: '<p>x</p>' });
  const open = { key: 's8', state: 'open', blocks: [approval.key], sent: { at: '', revision: 1, by: {} } };
  const panel = submissionPanel(open, [approval], where);
  root().append(panel);
  const reason = panel.querySelector('#reason-ship');
  reason.value = 'Too soon';
  reason.dispatchEvent(new Event('input'));
  const again = submissionPanel(open, [approval], where);
  root().append(again);
  assert.equal(again.querySelector('#reason-ship').value, 'Too soon');
  assert.equal(posted.length, 0);
  again.querySelector('#reject-ship').click();
  await new Promise((resolve) => setTimeout(resolve, 10));
  assert.deepEqual(posted[0].body.answers, [{ block: approval.key, verdict: 'rejected', reason: 'Too soon' }]);
  assert.equal(localStorage.getItem(`yard.reason.${approval.key}`), null);
});

test('answers that the page server refuses to record are kept in the browser', async () => {
  globalThis.fetch = async () => new Response(JSON.stringify({ outcome: 'error', errors: [{ kind: 'branch', message: 'the branch changed' }] }), { status: 409 });
  const asked = question('shelf', ['Top', 'Bottom']);
  const draft = choose({}, asked, 'Top');
  localStorage.setItem('yard.draft.s9', JSON.stringify(draft));
  const panel = submissionPanel({ key: 's9', state: 'open', blocks: [asked.key], sent: { at: '', revision: 1, by: {} } }, [asked], where);
  root().append(panel);
  panel.querySelector('#send-s9').click();
  await new Promise((resolve) => setTimeout(resolve, 10));
  assert.deepEqual(JSON.parse(localStorage.getItem('yard.draft.s9')), draft);
  assert.match(panel.querySelector('.send-message').textContent, /Not sent: the branch changed/);
  assert.equal(panel.querySelector('#send-s9').hasAttribute('disabled'), false);
});

test('a reason for a rejection that the page server refuses to record is kept in the browser', async () => {
  globalThis.fetch = async () => new Response(JSON.stringify({ outcome: 'error', errors: [{ kind: 'stopping', message: 'the page server is stopping' }] }), { status: 503 });
  const approval = block('approval', 'ship', { title: 'Ship it?', markdown: 'x', html: '<p>x</p>' });
  const panel = submissionPanel({ key: 's10', state: 'open', blocks: [approval.key], sent: { at: '', revision: 1, by: {} } }, [approval], where);
  root().append(panel);
  const reason = panel.querySelector('#reason-ship');
  reason.value = 'Too soon';
  reason.dispatchEvent(new Event('input'));
  panel.querySelector('#reject-ship').click();
  await new Promise((resolve) => setTimeout(resolve, 10));
  assert.equal(JSON.parse(localStorage.getItem(`yard.reason.${approval.key}`)), 'Too soon');
  assert.match(panel.querySelector('.decision-form .send-message').textContent, /Not sent: the page server is stopping/);
});

test('a send that cannot reach the page server says so, and its button can be pressed again', async () => {
  globalThis.fetch = async () => {
    throw new TypeError('Failed to fetch');
  };
  const asked = question('shelf', ['Top', 'Bottom']);
  localStorage.setItem('yard.draft.s11', JSON.stringify(choose({}, asked, 'Top')));
  const panel = submissionPanel({ key: 's11', state: 'open', blocks: [asked.key], sent: { at: '', revision: 1, by: {} } }, [asked], where);
  root().append(panel);
  panel.querySelector('#send-s11').click();
  await new Promise((resolve) => setTimeout(resolve, 10));
  assert.match(panel.querySelector('.send-message').textContent, /Not sent: the page server could not be reached/);
  assert.equal(panel.querySelector('#send-s11').hasAttribute('disabled'), false);
  assert.notEqual(localStorage.getItem('yard.draft.s11'), null);
});

test('approving sends the outcome approved', async () => {
  const approval = block('approval', 'ship', { title: 'Ship it?', markdown: 'x', html: '<p>x</p>' });
  const panel = submissionPanel({ key: 's6', state: 'open', blocks: [approval.key], sent: { at: '', revision: 1, by: {} } }, [approval], where);
  root().append(panel);
  panel.querySelector('#approve-ship').click();
  await new Promise((resolve) => setTimeout(resolve, 10));
  assert.deepEqual(posted[0].body.answers, [{ block: approval.key, verdict: 'approved' }]);
});

test('every plan has the same approval explanation regardless of host or native plan flag', () => {
  const explanations = [];
  for (const host of ['claude-code', 'codex']) {
    for (const planMode of [true, false, undefined]) {
      const body = { title: 'The plan', markdown: 'x', html: '<p>x</p>', ...(planMode === undefined ? {} : { planMode }) };
      const plan = block('plan', 'p1', body, { by: { host, session: 's1' } });
      const panel = submissionPanel({ key: 's7', state: 'open', blocks: [plan.key], sent: { at: '', revision: 1, by: {} } }, [plan], where);
      const text = panel.querySelector('.plan-mode')?.textContent;
      assert.match(text ?? '', /Approving authorizes the planned work, subject to the agent's mode and permissions/);
      assert.doesNotMatch(text, /auto mode/);
      explanations.push(text);
      assert.ok(panel.querySelector('#approve-p1'));
      assert.ok(panel.querySelector('#reject-p1'));
    }
  }
  assert.equal(new Set(explanations).size, 1);
  const approval = block('approval', 'a1', { title: 'Approve', markdown: 'x', html: '<p>x</p>' });
  const panel = submissionPanel({ key: 's8', state: 'open', blocks: [approval.key], sent: { at: '', revision: 1, by: {} } }, [approval], where);
  assert.equal(panel.querySelector('.plan-mode'), null);
});

test('an answer given elsewhere first is reported with the channel it came from', () => {
  const words = sentWords({ status: 200, body: { outcome: 'answered elsewhere', elsewhere: [{ id: 'shelf', channel: 'terminal' }] } });
  assert.match(words, /Answered elsewhere first, so the first answer is kept: shelf in the terminal/);
});
