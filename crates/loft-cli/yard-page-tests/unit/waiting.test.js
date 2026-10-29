// What waits on the person in the branch's Yard Log shown, in the panel along the bottom of the
// page: when it opens by itself, when it stays closed, how the person opens it again, and when it
// closes.

import { forget, root } from './dom.js';

import assert from 'node:assert/strict';
import { afterEach, beforeEach, test } from 'node:test';

import * as prefs from '../../yard-page/lib/prefs.js';
import { h } from '../../yard-page/lib/dom.js';
import * as modal from '../../yard-page/lib/modal.js';
import { logChrome, logView } from '../../yard-page/views/log.js';
import { openWaiting, waitingModal } from '../../yard-page/views/waiting.js';
import { EXPLORE, act, block, log, open, question, stage } from './fixtures.js';

const window = /** @type {any} */ (globalThis.window);

/** @type {any} */ (globalThis).mermaid = {
  initialize() {},
  async render(_id, text) {
    const escaped = text.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
    return { svg: `<svg viewBox="0 0 400 200"><text>${escaped}</text></svg>` };
  },
};

/** @type {string[]} */
let notices = [];
const fetched = globalThis.fetch;

beforeEach(() => {
  forget();
  notices = [];
  globalThis.fetch = async () => new Response(JSON.stringify({ outcome: 'answered', errors: [] }), { status: 200 });
});

afterEach(() => {
  globalThis.fetch = fetched;
});

const where = { tree: '/work/shelf', branch: 'feat/shelf', refresh: () => {}, notify: (text) => notices.push(text) };

/** The dialog of what waits. */
function dialog() {
  return /** @type {any} */ (document.querySelector('dialog.modal[data-modal="waiting"]'));
}

/** A Yard Log's page drawn into the document, with what waits brought into the panel, as the page does. */
function page(shown, more = {}) {
  const at = root();
  const context = {
    tree: '/work/shelf',
    branch: 'feat/shelf',
    last: null,
    now: Date.parse('2026-01-01T12:00:00Z'),
    width: 960,
    refresh: () => {},
    notify: () => {},
    onView: () => {},
    onPick: () => {},
    onWaiting: openWaiting,
    ...more,
  };
  at.append(h('header', { class: 'top' }, h('div', { id: 'chrome', class: 'chrome' }, logChrome(shown, context))), logView(shown, context));
  waitingModal(shown, where);
  return at;
}

/** The same Yard Log at a later revision, with `submissions` open. */
function later(shown, submissions, revision = shown.revision + 1) {
  return { ...shown, revision, submissions };
}

test('opening a branch’s Yard Log that has an open submission opens the panel, every open submission in it oldest first', () => {
  const first = question('colour', ['Red', 'Blue']);
  const second = question('size', ['Big', 'Small']);
  const shown = log([stage('s', 'S', 'active', [first, second])], {
    submissions: [open([second], '2026-01-01T10:09:00Z'), open([first], '2026-01-01T10:04:00Z', EXPLORE)],
  });
  page(shown);
  assert.ok(dialog().open);
  const panels = [...dialog().querySelectorAll('.modal-body .submission')];
  assert.equal(panels.length, 2);
  assert.deepEqual(panels.map((panel) => panel.querySelector('.question-form').dataset.key), [first.key, second.key]);
  assert.match(panels[0].querySelector('.sent').textContent, /Sent by Explore at/);
  assert.equal(dialog().querySelector('.modal-head h2').textContent, '2 blocks wait on you');
});

test('nothing of an open submission is shown above the timeline or the Gantt chart', () => {
  const asked = question('colour', ['Red', 'Blue']);
  const shown = log([stage('s', 'S', 'active', [asked])], { submissions: [open([asked], '2026-01-01T10:04:00Z')] });
  for (const view of ['gantt', 'timeline']) {
    prefs.set('view', view);
    const drawn = page(shown);
    assert.ok(!drawn.querySelector('.submission'), `nothing above the ${view}`);
    assert.ok(!drawn.querySelector('.question-form'), `no form above the ${view}`);
  }
});

test('where Yard Log has no open submission, no panel opens and nothing shows above the timeline', () => {
  const drawn = page(log([stage('s', 'S', 'active', [question('colour', ['Red', 'Blue'])])]));
  assert.ok(!modal.isOpen('waiting'));
  assert.ok(!drawn.querySelector('.submission'));
  assert.ok(!drawn.querySelector('#open-waiting'));
});

test('a closed, inherited or newer Yard Log opens no panel and offers nothing to answer', () => {
  const asked = question('colour', ['Red', 'Blue']);
  const submissions = [open([asked], '2026-01-01T10:04:00Z')];
  const yardLog = { key: 'k', title: 'Shelf', branch: 'feat/shelf', format: 'yardlog/1', closedBy: 'agent', closed: act('2026-01-01T11:00:00Z', 9) };
  for (const shown of [
    log([stage('s', 'S', 'done', [asked])], { submissions, yardLog }),
    log([stage('s', 'S', 'active', [asked])], { submissions, inherited: true }),
    log([stage('s', 'S', 'active', [asked])], { submissions, leftOut: 0 }),
  ]) {
    const drawn = page(shown);
    assert.ok(!modal.isOpen('waiting'));
    assert.ok(!drawn.querySelector('#open-waiting'));
  }
});

test('the panel closes by its close button, and by Escape while the focus is within it', () => {
  const asked = question('colour', ['Red', 'Blue']);
  const shown = log([stage('s', 'S', 'active', [asked])], { submissions: [open([asked], '2026-01-01T10:04:00Z')] });
  page(shown);
  assert.ok(dialog().classList.contains('modal--docked'));
  dialog().querySelector('.modal-close').click();
  assert.ok(!dialog().open);
  openWaiting();
  assert.ok(dialog().open);
  const escape = () => new window.KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true });
  document.body.dispatchEvent(escape());
  assert.ok(dialog().open, 'an Escape outside the panel leaves it open');
  dialog().querySelector('input').dispatchEvent(escape());
  assert.ok(!dialog().open);
});

test('a change to Yard Log that sends no new submission leaves the panel closed', () => {
  const asked = question('colour', ['Red', 'Blue']);
  const shown = log([stage('s', 'S', 'active', [asked])], { submissions: [open([asked], '2026-01-01T10:04:00Z')] });
  page(shown);
  modal.close('waiting');
  const note = block('note', 'n', { title: 'Meanwhile', markdown: 'x' }, { revision: 4 });
  waitingModal({ ...later(shown, shown.submissions), stages: [stage('s', 'S', 'active', [asked, note])] }, where);
  assert.ok(!dialog().open);
});

test('a submission sent after the panel was closed opens it again', () => {
  const first = question('colour', ['Red', 'Blue']);
  const second = question('size', ['Big', 'Small']);
  const shown = log([stage('s', 'S', 'active', [first, second])], { submissions: [open([first], '2026-01-01T10:04:00Z')] });
  page(shown);
  modal.close('waiting');
  waitingModal(later(shown, [...shown.submissions, open([second], '2026-01-01T10:09:00Z')]), where);
  assert.ok(dialog().open);
  assert.equal(dialog().querySelectorAll('.submission').length, 2);
});

test('a button in the page’s header, after the switch of views, gives the count of the blocks that wait and opens the panel', () => {
  const first = question('colour', ['Red', 'Blue']);
  const second = question('size', ['Big', 'Small']);
  const shown = log([stage('s', 'S', 'active', [first, second])], { submissions: [open([first, second], '2026-01-01T10:04:00Z')] });
  const drawn = page(shown);
  modal.close('waiting');
  const button = drawn.querySelector('header.top nav.views + #open-waiting');
  assert.equal(button.textContent, '2 wait on you');
  button.click();
  assert.ok(dialog().open);
});

test('each waiting block’s button to answer opens the panel at that block’s form', () => {
  const asked = question('colour', ['Red', 'Blue']);
  const approval = block('approval', 'ship', { title: 'Ship it', html: '<p>May I?</p>' });
  const shown = log([stage('s', 'S', 'active', [asked, approval])], { submissions: [open([asked, approval], '2026-01-01T10:04:00Z')] });
  const drawn = page(shown);
  modal.close('waiting');
  drawn.querySelector(`article[data-key="${approval.key}"] button.link`).click();
  assert.ok(dialog().open);
  const form = dialog().querySelector(`.decision-form[data-key="${approval.key}"]`);
  assert.ok(form);
  assert.ok(form.contains(document.activeElement), 'the focus is in the block’s form');
});

test('what was chosen or typed and not sent is shown again when the panel opens again', () => {
  const asked = question('colour', ['Red', 'Blue'], { ownWords: true });
  const shown = log([stage('s', 'S', 'active', [asked])], { submissions: [open([asked], '2026-01-01T10:04:00Z')] });
  page(shown);
  const blue = dialog().querySelector('input[value="Blue"]');
  blue.checked = true;
  blue.dispatchEvent(new Event('change'));
  const words = dialog().querySelector(`#words-${asked.id}`);
  words.value = 'Or teal';
  words.dispatchEvent(new Event('input'));
  modal.close('waiting');
  openWaiting();
  assert.equal(dialog().querySelector('input[value="Blue"]').checked, false);
  assert.equal(dialog().querySelector(`#answer-${asked.id}-words`).checked, true);
  assert.equal(dialog().querySelector(`#words-${asked.id}`).value, 'Or teal');
});

test('accepting the recommendations shows them chosen in the panel at once', () => {
  const asked = question('colour', ['Red', 'Blue']);
  const shown = log([stage('s', 'S', 'active', [asked])], { submissions: [open([asked], '2026-01-01T10:04:00Z')] });
  page(shown);
  const accept = [...dialog().querySelectorAll('button')].find((button) => button.textContent === 'Accept the recommendations');
  accept.click();
  const recommended = asked.version.body.options.find((option) => option.recommended).label;
  assert.ok(dialog().querySelector(`input[value="${recommended}"]`).checked);
});

test('the panel closes by itself once nothing in it waits on the person, and the page then says what was sent', async () => {
  const asked = question('colour', ['Red', 'Blue']);
  const shown = log([stage('s', 'S', 'active', [asked])], { submissions: [open([asked], '2026-01-01T10:04:00Z')] });
  page(shown);
  const blue = dialog().querySelector('input[value="Blue"]');
  blue.checked = true;
  blue.dispatchEvent(new Event('change'));
  dialog().querySelector('button.primary').click();
  await new Promise((done) => setTimeout(done, 0));
  assert.deepEqual(notices, ['Sent.']);
  waitingModal(later(shown, []), where);
  assert.ok(!dialog().open);
});

test('a diagram in a waiting block is drawn in the panel, as it is in the timeline', async () => {
  const asked = question('colour', ['Red', 'Blue'], {
    contextHtml: '<pre><code class="language-mermaid">flowchart LR\n  A --&gt; B</code></pre>',
  });
  const shown = log([stage('s', 'S', 'active', [asked])], { submissions: [open([asked], '2026-01-01T10:04:00Z')] });
  page(shown);
  for (let turn = 0; turn < 5; turn += 1) await new Promise((done) => setTimeout(done, 0));
  const body = dialog().querySelector('.modal-body');
  assert.ok(!body.querySelector('pre > code.language-mermaid'), 'the diagram is still a code block');
  assert.equal(body.querySelector('.diagram svg text')?.textContent, 'flowchart LR\n  A --> B');
});
