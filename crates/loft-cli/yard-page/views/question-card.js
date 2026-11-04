// A question as the page draws it, in the timeline and in the panel of what waits alike: a head
// that gives its header as a chip and its state at the right, its prompt as its title with its
// context beneath, and its options as cards side by side, each lettered A, B, C and on in a
// circle before its label, a recommended one marked beneath its label, and its detail beneath,
// at most three to a row; the person's own words are a card of their own beneath them. The
// panel's cards choose, and a button beside the state clears the choice; the timeline's only show.

import { h } from '../lib/dom.js';

/**
 * The letter of the option at `index`: A to Z, then AA, AB and on.
 * @param {number} index
 */
export function letter(index) {
  let text = '';
  for (let n = index; n >= 0; n = Math.floor(n / 26) - 1) text = String.fromCharCode(65 + (n % 26)) + text;
  return text;
}

/**
 * The head of a question: its kind, its header as a chip, and its state at the right.
 * @param {string} header
 * @param {string} state
 * @param {string} [tone] `chosen`, `answered` or `waiting`
 * @param {Node} [after] what follows the state, as the button that clears the choice
 */
export function questionHead(header, state, tone, after) {
  return h(
    'header',
    { class: 'block-head question-head' },
    h('span', { class: 'kind' }, 'Question'),
    header ? h('h4', { class: 'question-chip' }, header) : null,
    h('span', { class: ['question-state', tone ? `question-state--${tone}` : null], 'aria-live': 'polite' }, state),
    after ?? null,
  );
}

/**
 * The words of a state that names the options chosen by their letters: `Chosen: B`.
 * @param {string[]} letters
 */
export function chosenState(letters) {
  return letters.length ? `Chosen: ${letters.join(', ')}` : 'Not chosen yet';
}

/**
 * An option's card: `tag` with `attributes`, the option's letter in a circle before its label,
 * the recommended mark beneath the label, then its detail and anything else it holds.
 * @param {string} tag
 * @param {Record<string, any>} attributes
 * @param {{ index: number, label: Node, recommended?: boolean, chosen?: boolean, before?: any, after?: any[] }} option
 */
export function optionCard(tag, attributes, { index, label, recommended, chosen, before, after = [] }) {
  return h(
    tag,
    { ...attributes, class: [attributes.class, 'option-card', chosen ? 'is-chosen' : null], dataset: { letter: letter(index), ...(attributes.dataset ?? {}) } },
    before,
    h('span', { class: 'option-top' }, h('span', { class: 'option-letter', 'aria-hidden': 'true' }, letter(index)), label),
    recommended ? h('span', { class: 'badge badge--recommended' }, 'Recommended') : null,
    ...after,
  );
}

/**
 * The grid the cards of a question's options stand in.
 * @param {any[]} cards
 * @param {string} [tag]
 */
export function optionGrid(cards, tag = 'div') {
  return h(tag, { class: 'options option-grid' }, cards);
}
