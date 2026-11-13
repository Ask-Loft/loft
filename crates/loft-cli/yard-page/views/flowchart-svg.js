// A laid-out flowchart drawn as the page's own SVG: its groups as panels framed by a hairline and
// titled in small muted text, then its edges in muted ink, each turning with rounded elbows and
// ending in a small arrowhead, their labels in pills, and its nodes as flat cards in their shapes.
// Colors come from the page's stylesheet through classes, so that a change of scheme recolors a
// flowchart without laying it out again; only a node's own `style` or `classDef` is written on it.

import { svg } from '../lib/dom.js';
import { PAD, TITLE, pathOf } from '../model/flowchart.js';
import { cardSvg, panelSvg } from './spec-svg.js';

/** The room around the flowchart, and the radius of an edge's elbows, in pixels. */
const MARGIN = 8;
const ELBOW = 6;

/**
 * A label's lines, centered on (`x`, `y`).
 * @param {string[]} lines
 * @param {number} x
 * @param {number} y
 * @param {string} kind
 */
function label(lines, x, y, kind) {
  const top = y - ((lines.length - 1) * PAD.line) / 2;
  return svg(
    'text',
    { class: kind, x, y: top, 'text-anchor': 'middle', 'dominant-baseline': 'central' },
    lines.map((line, index) => svg('tspan', { x, y: top + index * PAD.line }, line)),
  );
}

/**
 * A node's shape, in its box.
 * @param {any} node
 */
function shapeOf(node) {
  const { x, y, width, height } = node;
  switch (node.shape) {
    case 'diamond':
      return svg('polygon', { points: `${x + width / 2},${y} ${x + width},${y + height / 2} ${x + width / 2},${y + height} ${x},${y + height / 2}` });
    case 'circle':
      return svg('circle', { cx: x + width / 2, cy: y + height / 2, r: width / 2 });
    case 'stadium':
      return svg('rect', { x, y, width, height, rx: height / 2, ry: height / 2 });
    case 'round':
      return svg('rect', { x, y, width, height, rx: 8, ry: 8 });
    default:
      return svg('rect', { x, y, width, height });
  }
}

/**
 * A node: its shape and its label, with the colors its own style gives it.
 * @param {any} node
 */
function nodeOf(node) {
  const style = node.style ?? {};
  const shape = shapeOf(node);
  const fills = [style.fill ? `fill:${style.fill}` : '', style.stroke ? `stroke:${style.stroke}` : ''].filter(Boolean).join(';');
  if (fills) shape.setAttribute('style', fills);
  const text = label(node.lines, node.x + node.width / 2, node.y + node.height / 2, 'flow-label');
  if (style.color) text.setAttribute('style', `fill:${style.color}`);
  return svg('g', { class: `flow-node flow-node--${node.shape}`, 'data-id': node.id }, shape, text);
}

/**
 * An edge: its path, and its label in a pill.
 * @param {any} edge
 * @param {string} marker the id of the arrowhead's marker
 */
function edgeOf(edge, marker) {
  const ends = { 'data-from': edge.from, 'data-to': edge.to };
  const path = svg('path', {
    class: `flow-edge flow-edge--${edge.pattern}`,
    ...ends,
    d: pathOf(edge.points, ELBOW),
    'marker-end': edge.arrowEnd ? `url(#${marker})` : null,
    'marker-start': edge.arrowStart ? `url(#${marker}-start)` : null,
  });
  const pill = edge.label
    ? svg(
        'g',
        { class: 'flow-pill', ...ends },
        svg('rect', { x: edge.label.x, y: edge.label.y, width: edge.label.width, height: edge.label.height, rx: edge.label.height / 2, ry: edge.label.height / 2 }),
        label(edge.label.lines, edge.label.x + edge.label.width / 2, edge.label.y + edge.label.height / 2, 'flow-pill-label'),
      )
    : null;
  return { path, pill };
}

/**
 * The arrowheads of a flowchart: one for an edge's end and one, turned about, for its start.
 * @param {string} id
 */
function markers(id) {
  const head = (/** @type {string} */ name, /** @type {string} */ d) =>
    svg('marker', { id: name, viewBox: '0 0 10 10', refX: 9, refY: 5, markerWidth: 7, markerHeight: 7, markerUnits: 'userSpaceOnUse', orient: 'auto' }, svg('path', { class: 'flow-arrow', d }));
  return svg('defs', {}, head(id, 'M0 1L10 5L0 9z'), head(`${id}-start`, 'M10 1L0 5L10 9z'));
}

/**
 * The flowchart `layout` gives, as an SVG of its own size.
 * @param {ReturnType<typeof import('../model/flowchart.js').placed>} layout
 * @param {string} id unique on the page, for the arrowheads' markers
 * @param {string} [name] what the diagram is, for a reader
 */
export function flowchartSvg(layout, id, name) {
  const width = layout.width + 2 * MARGIN;
  const height = layout.height + 2 * MARGIN;
  const marker = `${id}-arrow`;
  const edges = layout.edges.map((edge) => edgeOf(edge, marker));
  return svg(
    'svg',
    {
      class: 'flow',
      id,
      viewBox: `${-MARGIN} ${-MARGIN} ${width} ${height}`,
      width,
      height,
      role: 'img',
      'aria-label': name ?? `Flowchart of ${layout.nodes.length} ${layout.nodes.length === 1 ? 'node' : 'nodes'}`,
    },
    markers(marker),
    layout.groups.map((group) =>
      group.panel ? panelSvg(group) : svg(
        'g',
        { class: 'flow-group', 'data-id': group.id },
        svg('rect', { x: group.x, y: group.y, width: group.width, height: group.height }),
        svg('text', { class: 'flow-group-title', x: group.x + TITLE.side, y: group.y + 22 }, group.title),
      ),
    ),
    edges.map((edge) => edge.path),
    layout.nodes.map((node) => (node.card ? cardSvg(node) : nodeOf(node))),
    edges.map((edge) => edge.pill),
  );
}
