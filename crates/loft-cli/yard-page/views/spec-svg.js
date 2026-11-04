// The cards and panels of a `spec-change` view's diagram, drawn into the page's own flowchart SVG.
// A card is a flat frame with a hairline border, a bar at its left in its change's colour, its
// kind's square and its label in monospace, its change in a word at its right, and its name in
// the page's face beneath; a panel is a frame that holds its nodes, headed the same way. Colours
// come from the stylesheet through classes, so that a change of scheme only recolours. A card or
// panel whose node changed, and a folded Feature Group, can be chosen.

import { svg } from '../lib/dom.js';
import { PAD } from '../model/flowchart.js';
import { CARD, KIND_NAMES, PANEL } from '../model/spec-graph.js';

/**
 * The attributes that let a node be chosen and name it to a reader.
 * @param {{ key: string, kind: string, label: string, word: string | null, chosen: boolean }} what
 * @param {string} name
 */
function choosable(what, name) {
  if (!what.chosen) return {};
  const kind = KIND_NAMES[/** @type {keyof typeof KIND_NAMES} */ (what.kind)] ?? what.kind;
  const said = [what.label === kind ? kind : `${what.label}, ${kind}`, what.word ? what.word.toLowerCase() : null].filter(Boolean).join(', ');
  return { 'data-key': what.key, role: 'button', tabindex: 0, 'aria-label': `${said}: ${name}` };
}

/**
 * A box's head: its kind's square and its label at the left, and its change's word at the right.
 * @param {{ kind: string, label: string, word: string | null }} what
 * @param {number} left
 * @param {number} right
 * @param {number} y the head's middle
 * @param {string} [name] a panel's name, after its label
 */
function head(what, left, right, y, name) {
  return [
    svg('rect', { class: `spec-kind spec-kind--${what.kind}`, x: left, y: y - 3, width: 6, height: 6 }),
    svg(
      'text',
      { x: left + 12, y, 'dominant-baseline': 'central' },
      svg('tspan', { class: 'spec-label' }, what.label),
      name ? svg('tspan', { class: 'spec-panel-name', dx: 8 }, name) : null,
    ),
    what.word ? svg('text', { class: 'spec-word', x: right, y, 'text-anchor': 'end', 'dominant-baseline': 'central' }, what.word) : null,
  ];
}

/**
 * A card, in its place.
 * @param {any} node a placed node with its `card`
 */
export function cardSvg(node) {
  const { x, y, width, height, card } = node;
  const left = x + CARD.bar + CARD.pad;
  const middle = y + CARD.pad + CARD.head / 2;
  const first = y + CARD.pad + CARD.head + CARD.gap + PAD.line / 2;
  const below = y + CARD.pad + CARD.head + (card.lines.length ? CARD.gap + card.lines.length * PAD.line : 0) + CARD.extra / 2;
  return svg(
    'g',
    { class: `spec-card spec-card--${card.change ?? 'context'}`, 'data-id': node.id, ...choosable(card, card.lines.join(' ')) },
    svg('rect', { class: 'spec-frame', x, y, width, height }),
    card.change ? svg('rect', { class: 'spec-bar', x, y, width: CARD.bar, height }) : null,
    head(card, left, x + width - CARD.pad, middle),
    card.lines.length
      ? svg('text', { class: 'flow-label spec-name' }, card.lines.map((line, index) => svg('tspan', { x: left, y: first + index * PAD.line, 'dominant-baseline': 'central' }, line)))
      : null,
    card.extras.length
      ? svg('text', { class: 'spec-extra' }, card.extras.map((line, index) => svg('tspan', { x: left, y: below + index * CARD.extra, 'dominant-baseline': 'central' }, line)))
      : null,
  );
}

/**
 * A panel, in its place, beneath the nodes it holds.
 * @param {any} group a placed group with its `panel`
 */
export function panelSvg(group) {
  const { x, y, width, height, panel } = group;
  return svg(
    'g',
    {
      class: `spec-panel spec-panel--${panel.change ?? 'context'} spec-panel--${panel.depth % 2 ? 'odd' : 'even'}`,
      'data-panel': group.id,
      ...choosable(panel, panel.name),
    },
    svg('rect', { class: 'spec-panel-frame', x, y, width, height }),
    panel.change ? svg('rect', { class: 'spec-bar', x, y, width: CARD.bar, height }) : null,
    head(panel, x + CARD.bar + PANEL.pad, x + width - PANEL.pad, y + PANEL.head / 2, panel.name),
  );
}
