// A question drawn as a card, in the panel of what waits and in the timeline alike: its head with
// its header and its state, its prompt as its title, and its options as lettered cards.

import { forget, root } from './dom.js';

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { beforeEach, test } from 'node:test';

import { clear } from '../../yard-page/model/answers.js';
import { blocksByKey, decisionsByQuestion } from '../../yard-page/model/log.js';
import { submissionPanel } from '../../yard-page/views/answering.js';
import { card } from '../../yard-page/views/blocks.js';
import { letter } from '../../yard-page/views/question-card.js';
import { answer, log, question, stage } from './fixtures.js';

beforeEach(forget);

const where = { tree: '/work/shelf', branch: 'feat/shelf', refresh() {}, notify() {} };
const css = readFileSync(new URL('../../yard-page/yard.css', import.meta.url), 'utf8');

/** The panel of a submission of `q`, drawn on the page. */
function panelOf(q) {
  const panel = submissionPanel({ key: 'cards', state: 'open', blocks: [q.key], sent: q.version.act }, [q], where);
  root().replaceChildren(panel);
  return panel;
}

/** `q` drawn in the timeline, with `waiting` the keys of what waits on the person. */
function inTimeline(q, waiting = new Set()) {
  const shown = log([stage('s', 'S', 'active', [q])]);
  return card(q, { blocks: blocksByKey(shown), decisions: decisionsByQuestion(shown), marked: new Set(), waiting });
}

test('the head of a question gives its header in small monospace capitals and, at its right, its state', () => {
  const q = question('shelf', ['Top', 'Bottom']);
  const panel = panelOf(q);
  assert.equal(panel.querySelector('.question-head .question-chip')?.textContent, q.version.body.header);
  const state = () => panel.querySelector('.question-head .question-state')?.textContent;
  assert.equal(state(), 'Not chosen yet');
  /** @type {HTMLInputElement} */ (panel.querySelector('input[value="Bottom"]')).click();
  assert.equal(state(), 'Chosen: B', 'the options chosen, by their letters');
  q.answer = answer(['Top'], 'page');
  assert.equal(inTimeline(q).querySelector('.question-state')?.textContent, 'Top', 'its answer once answered');
  assert.equal(inTimeline(question('open', ['A', 'B'])).querySelector('.question-state')?.textContent, 'Not chosen yet');
  assert.match(css, /\.question-chip \{[^}]*font-family: var\(--mono\);[^}]*text-transform: uppercase;/);
  assert.match(css, /\.question-state \{ margin-left: auto;/);
});

test('a question’s prompt is its title, and its context lies beneath it', () => {
  const q = question('shelf', ['Top', 'Bottom'], { contextHtml: '<p>Some context</p>' });
  for (const drawn of [panelOf(q), inTimeline(q)]) {
    const prompt = drawn.querySelector('.prompt');
    assert.equal(prompt?.textContent, 'Which shelf?');
    assert.ok(prompt && prompt.compareDocumentPosition(/** @type {Node} */ (drawn.querySelector('.context'))) & Node.DOCUMENT_POSITION_FOLLOWING);
  }
});

test('a question’s prompt is set as a block’s title is, at 15 pixels and semibold', () => {
  assert.match(css, /\.prompt \{[^}]*font-size: 15px; font-weight: 600;/);
  assert.match(css, /\.block-head h4 \{ font-size: 15px; font-weight: 600;/, 'as a block’s title');
});

test('a question’s header chip, its state, its options’ labels and its answer are no larger than 14 pixels', () => {
  const size = (rule) => Number(new RegExp(`${rule} \\{[^}]*font-size: (\\d+)px`).exec(css)?.[1]);
  for (const rule of ['\\.question-head \\.question-chip', '\\.question-state', ':is\\(\\.option-label, \\.choice-label\\)', '\\.answer-text']) {
    assert.ok(size(rule) <= 14, `${rule} at ${size(rule)}px`);
  }
  // The block's title rule does not outrank the chip's, which once made it a title's size.
  assert.match(css, /\.question-head \.question-chip \{/);
});

test('nothing in a question is bolder than semibold', () => {
  const rules = css.match(/^[^{}\n]*(?:question|prompt|option|choice|answer-text|clear-choice)[^{}\n]*\{[^}]*\}/gm) ?? [];
  assert.ok(rules.length > 8);
  for (const rule of rules) {
    for (const [, weight] of rule.matchAll(/font-weight: (\d+)/g)) assert.ok(Number(weight) <= 600, rule);
  }
});

test('each option is a card lettered A, B, C and on in a circle before its label, its detail beneath, a recommended one marked', () => {
  const q = question('shelf', ['Top', 'Middle', 'Bottom']);
  q.version.body.options = q.version.body.options.map((option, index) => ({ ...option, recommended: index === 1, detail: `About ${option.label}`, detailHtml: `<p>About ${option.label}</p>` }));
  for (const drawn of [panelOf(q), inTimeline(q)]) {
    const cards = [...drawn.querySelectorAll('.option-grid > .option-card')];
    assert.deepEqual(cards.slice(0, 3).map((each) => each.querySelector('.option-letter')?.textContent), ['A', 'B', 'C']);
    assert.deepEqual(cards.slice(0, 3).map((each) => each.querySelector('.option-top :is(.option-label, .choice-label)')?.textContent), ['Top', 'Middle', 'Bottom']);
    assert.deepEqual(cards.slice(0, 3).map((each) => Boolean(each.querySelector('.badge--recommended'))), [false, true, false]);
    const middle = cards[1];
    assert.ok(middle.querySelector('.option-top')?.compareDocumentPosition(/** @type {Node} */ (middle.querySelector('.badge--recommended'))) & Node.DOCUMENT_POSITION_FOLLOWING, 'marked beneath its label');
    assert.equal(middle.querySelector(':is(.option-detail, .choice-detail)')?.textContent, 'About Middle');
  }
  assert.deepEqual([0, 25, 26, 27, 51, 52].map(letter), ['A', 'Z', 'AA', 'AB', 'AZ', 'BA']);
  assert.match(css, /\.option-letter \{[^}]*border-radius: 50%;/);
});

test('the person’s own words are a card of their own beneath the options’ cards, as wide as they are, lettered after them and holding their text field', () => {
  const q = question('shelf', ['Top', 'Bottom']);
  const panel = panelOf(q);
  assert.ok(!panel.querySelector('.option-grid > .own-words'), 'not among the options’ cards');
  const own = panel.querySelector('.option-grid + .own-words');
  assert.ok(own?.classList.contains('option-card'), 'beneath them, a card of its own');
  assert.equal(own?.querySelector('.option-letter')?.textContent, 'C');
  assert.ok(own?.querySelector('textarea'), 'its text field inside it');
  /** @type {HTMLInputElement} */ (panel.querySelector(`#answer-${q.id}-words`)).click();
  assert.equal(panel.querySelector('.question-state')?.textContent, 'Chosen: C');
});

test('the cards of the options stand side by side, at most three to a row, and one beneath another when narrow', () => {
  assert.match(css, /\.option-grid \{[^}]*grid-template-columns: repeat\(auto-fit, minmax\(max\(220px, calc\(\(100% - 24px\) \/ 3\)\), 1fr\)\); gap: 12px;/);
  assert.match(css, /@media \(max-width: 640px\) \{ \.option-grid \{ grid-template-columns: minmax\(0, 1fr\); \} \}/, 'one beneath another in a narrow window');
});

test('the card of a chosen option is outlined in action blue', () => {
  const q = question('shelf', ['Top', 'Bottom']);
  q.answer = answer(['Bottom'], 'page');
  const drawn = inTimeline(q);
  assert.deepEqual([...drawn.querySelectorAll('.option-card')].map((each) => each.classList.contains('is-chosen')), [false, true]);
  assert.match(css, /\.option-card\.is-chosen, \.option-card:has\(\.option-input:checked\) \{ outline: 2px solid var\(--signal\);/);
});

test('in the timeline, a question’s cards offer no control that chooses an option', () => {
  const q = question('shelf', ['Top', 'Bottom']);
  const drawn = inTimeline(q, new Set([q.key]));
  assert.equal(drawn.querySelectorAll('.option-card').length, 2);
  assert.ok(!drawn.querySelector('.option-grid input, .option-grid textarea, .option-grid button'));
  assert.equal(drawn.querySelector('.question-state')?.textContent, 'Waits on you');
});

test('clearing a question’s choice deselects its options and its own words, and keeps the text the person typed', () => {
  const q = question('shelf', ['Top', 'Bottom']);
  assert.deepEqual(clear({ [q.key]: { labels: ['Top'], words: 'Teal', ownWordsSelected: true } }, q)[q.key], { labels: [], words: 'Teal', ownWordsSelected: false });
  const panel = panelOf(q);
  /** @type {HTMLInputElement} */ (panel.querySelector('input[value="Top"]')).click();
  const words = /** @type {HTMLTextAreaElement} */ (panel.querySelector(`#words-${q.id}`));
  words.value = 'Teal';
  words.dispatchEvent(new Event('input', { bubbles: true }));
  /** @type {HTMLElement} */ (panel.querySelector('.clear-choice')).click();
  assert.ok(![...panel.querySelectorAll('.option-input')].some((input) => /** @type {HTMLInputElement} */ (input).checked), 'nothing chosen');
  assert.equal(words.value, 'Teal', 'the text kept');
  assert.equal(panel.querySelector('.question-state')?.textContent, 'Not chosen yet');
});

test('a button beside the question’s state clears its choice, disabled while nothing is chosen', () => {
  const q = question('shelf', ['Top', 'Bottom']);
  const panel = panelOf(q);
  const button = /** @type {HTMLButtonElement} */ (panel.querySelector('.question-head .question-state + button.clear-choice'));
  assert.ok(button, 'in the head, beside the state');
  assert.equal(button.disabled, true);
  /** @type {HTMLInputElement} */ (panel.querySelector('input[value="Top"]')).click();
  assert.equal(button.disabled, false);
  button.click();
  assert.equal(button.disabled, true);
  assert.equal(panel.querySelector('.question-state')?.textContent, 'Not chosen yet');
  assert.ok(!panel.querySelector('.question-foot, .link.clear-choice'), 'no link beneath the options');
});

test('only the panel’s title says that blocks wait: a submission in it has no heading of its own', () => {
  const panel = panelOf(question('shelf', ['Top', 'Bottom']));
  assert.ok(!panel.querySelector('.submission-head h3, .submission-head h2'));
  assert.doesNotMatch(panel.querySelector('.submission-head')?.textContent ?? '', /wait/i);
  assert.match(panel.querySelector('.submission-head .sent')?.textContent ?? '', /^Sent by /);
});

test('a question in the panel is headed by its prompt, above its header chip and its state, and its prompt names its choices', () => {
  const q = question('shelf', ['Top', 'Bottom']);
  const panel = panelOf(q);
  const card = panel.querySelector('.question-card');
  assert.equal(card?.firstElementChild?.matches('h3.prompt'), true, 'the prompt first');
  assert.equal(card?.firstElementChild?.nextElementSibling?.matches('.question-head'), true, 'then the head');
  const prompt = /** @type {HTMLElement} */ (card?.firstElementChild);
  assert.equal(card?.querySelector('fieldset')?.getAttribute('aria-labelledby'), prompt.id);
});
