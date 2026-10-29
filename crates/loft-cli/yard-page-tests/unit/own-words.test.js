// Free-form text and the explicit option choice are distinct browser draft state.
import { forget, root } from './dom.js';
import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';
import { acceptRecommendations, choose, readyToSend, say, toSend } from '../../yard-page/model/answers.js';
import { submissionPanel } from '../../yard-page/views/answering.js';
import { question } from './fixtures.js';

beforeEach(forget);
const where = { tree: '/work/shelf', branch: 'feat/shelf', refresh() {}, notify() {} };
const own = q => `#answer-${q.id}-words`;
const words = q => `#words-${q.id}`;
const saved = () => JSON.parse(localStorage.getItem('yard.draft.own') ?? '{}');
function draw(q, draft) {
  if (draft) localStorage.setItem('yard.draft.own', JSON.stringify(draft));
  const panel = submissionPanel({ key: 'own', state: 'open', blocks: [q.key], sent: q.version.act }, [q], where);
  root().replaceChildren(panel);
  return panel;
}
function edit(panel, q, value) {
  const input = panel.querySelector(words(q));
  input.value = value;
  input.dispatchEvent(new Event('input', { bubbles: true }));
}

test('typing selects the own words radio and clears named choices without replacing the text field', () => {
  const q = question('shelf', ['Red', 'Blue']);
  const panel = draw(q);
  panel.querySelector('input[value="Red"]').click();
  const input = panel.querySelector(words(q));
  input.focus();
  input.value = 'Teal';
  input.setSelectionRange(2, 2);
  input.dispatchEvent(new Event('input', { bubbles: true }));
  assert.equal(panel.querySelector(own(q))?.type, 'radio');
  assert.equal(panel.querySelector(own(q)).name, panel.querySelector('input[value="Red"]').name);
  assert.equal(panel.querySelector(own(q)).checked, true);
  assert.equal(panel.querySelector('input[value="Red"]').checked, false);
  assert.deepEqual(saved()[q.key].labels, []);
  assert.equal(saved()[q.key].ownWordsSelected, true);
  assert.equal(document.activeElement, input);
  assert.equal(panel.querySelector(words(q)), input);
  assert.equal(input.selectionStart, 2);
});

test('a named radio deselects own words while preserving text that is excluded from sending', () => {
  const q = question('shelf', ['Red', 'Blue']);
  const draft = choose(say({}, q, 'Teal'), q, 'Blue');
  assert.equal(draft[q.key].ownWordsSelected, false);
  assert.equal(draft[q.key].words, 'Teal');
  assert.deepEqual(toSend(draft, [q]), [{ block: q.key, labels: ['Blue'] }]);
  const panel = draw(q, draft);
  panel.querySelector(own(q)).click();
  assert.equal(panel.querySelector('input[value="Blue"]').checked, false);
  assert.deepEqual(toSend(saved(), [q]), [{ block: q.key, labels: [], words: 'Teal' }]);
  panel.querySelector('input[value="Red"]').click();
  assert.equal(panel.querySelector(own(q)).checked, false);
  assert.equal(panel.querySelector(words(q)).value, 'Teal');
  assert.deepEqual(toSend(saved(), [q]), [{ block: q.key, labels: ['Red'] }]);
});

test('the own words checkbox combines with named choices and can be unchecked without losing text', () => {
  const q = question('shelf', ['Red', 'Blue'], { multiple: true });
  const panel = draw(q);
  panel.querySelector('input[value="Red"]').click();
  edit(panel, q, 'Teal');
  const control = panel.querySelector(own(q));
  assert.equal(control?.type, 'checkbox');
  assert.equal(control.checked, true);
  assert.equal(panel.querySelector('input[value="Red"]').checked, true);
  assert.deepEqual(toSend(saved(), [q]), [{ block: q.key, labels: ['Red'], words: 'Teal' }]);
  control.click();
  assert.equal(panel.querySelector(words(q)).value, 'Teal');
  assert.deepEqual(toSend(saved(), [q]), [{ block: q.key, labels: ['Red'] }]);
  control.click();
  panel.querySelector('input[value="Blue"]').click();
  assert.equal(control.checked, true);
  assert.deepEqual(toSend(saved(), [q]), [{ block: q.key, labels: ['Red', 'Blue'], words: 'Teal' }]);
});

test('clearing text keeps the explicit choice selected and prevents sending even alongside named checkboxes', () => {
  for (const multiple of [false, true]) {
    const q = question('shelf', ['Red', 'Blue'], { multiple });
    const panel = draw(q);
    if (multiple) panel.querySelector('input[value="Red"]').click();
    panel.querySelector(own(q))?.click();
    assert.equal(panel.querySelector(own(q)).checked, true);
    assert.equal(panel.querySelector('#send-own').disabled, true);
    edit(panel, q, 'Teal');
    edit(panel, q, '   ');
    assert.equal(panel.querySelector(own(q)).checked, true);
    assert.equal(readyToSend(saved(), [q]), false);
    assert.equal(panel.querySelector('#send-own').disabled, true);
  }
});

test('recommendations preserve a selected empty own words option but ignore unselected draft text', () => {
  const q = question('shelf', ['Red', 'Blue']);
  const selected = { [q.key]: { labels: [], words: '', ownWordsSelected: true } };
  assert.deepEqual(acceptRecommendations(selected, [q]), selected);
  const legacy = { [q.key]: { labels: [], words: 'Teal' } };
  const recommended = acceptRecommendations(legacy, [q]);
  assert.deepEqual(recommended[q.key].labels, ['Red']);
  assert.equal(recommended[q.key].words, 'Teal');
  assert.deepEqual(toSend(recommended, [q]), [{ block: q.key, labels: ['Red'] }]);
});

test('restoring a draft uses explicit selection independently of retained text including legacy drafts', () => {
  for (const multiple of [false, true]) {
    const q = question('shelf', ['Red', 'Blue'], { multiple });
    for (const selected of [undefined, false, true, 'legacy']) {
      for (const text of ['', 'Teal']) {
        const draft = { [q.key]: { labels: [], words: text, ...(selected === undefined ? {} : { ownWordsSelected: selected }) } };
        const panel = draw(q, draft);
        assert.equal(panel.querySelector(own(q))?.checked, selected === true);
        assert.equal(panel.querySelector(words(q)).value, text);
        assert.equal(panel.querySelector('#send-own').disabled, !(selected === true && text !== ''));
      }
    }
  }
});

test('editing restored unselected text selects the option and sends only the existing answer wire fields', () => {
  const q = question('shelf', ['Red', 'Blue']);
  const panel = draw(q, { [q.key]: { labels: ['Red'], words: 'Teal', ownWordsSelected: false } });
  edit(panel, q, ' Green ');
  assert.equal(panel.querySelector(own(q))?.checked, true);
  assert.equal(panel.querySelector('input[value="Red"]').checked, false);
  assert.deepEqual(toSend(saved(), [q]), [{ block: q.key, labels: [], words: 'Green' }]);
});
