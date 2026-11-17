// A Mermaid flowchart laid out by the vendored ELK and drawn by the page: the graph made of what
// Mermaid's parser gives, ELK's answer placed in the flowchart's frame, and the SVG drawn from it.

import { forget } from './dom.js';

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { beforeEach, test } from 'node:test';
import { gunzipSync } from 'node:zlib';

import { FLOWCHART } from '../../yard-page/lib/flowchart.js';
import { graphOf, lines, pathOf, placed, sizeOf, styleOf } from '../../yard-page/model/flowchart.js';
import { flowchartSvg } from '../../yard-page/views/flowchart-svg.js';

beforeEach(forget);

/** The ELK the page server serves, run as the page runs it. */
const ELK = (() => {
  const code = gunzipSync(readFileSync(new URL('../../yard-page/vendor/elk.bundled.js.gz', import.meta.url))).toString('utf8');
  const module = { exports: {} };
  new Function('module', 'exports', code)(module, module.exports);
  return /** @type {any} */ (module.exports);
})();

/** A label's size at 7 pixels a character and 18 a line. @param {string[]} text */
const measure = (text) => ({ width: Math.max(...text.map((line) => line.length * 7)), height: text.length * 18 });

/** What Mermaid's parser gives for a flowchart of `direction`: start, a decision, a group of one. */
function parsed(direction = 'LR') {
  return {
    direction,
    classes: { hot: ['fill:#fdd', 'stroke:#c00', 'color:#600'] },
    data: {
      nodes: [
        { id: 'S', label: 'The group', shape: 'rect', isGroup: true },
        { id: 'A', label: 'Start here', shape: 'squareRect', cssStyles: ['fill:#eef'], cssClasses: 'default ' },
        { id: 'B', label: 'Ok?', shape: 'diamond', cssStyles: [], cssClasses: 'default ' },
        { id: 'C', label: 'circle', shape: 'circle', parentId: 'S', cssStyles: [], cssClasses: 'default ' },
        { id: 'D', label: 'one<br/>two', shape: 'stadium', cssStyles: [], cssClasses: 'default ' },
        { id: 'E', label: 'round', shape: 'roundedRect', cssStyles: [], cssClasses: 'default hot' },
      ],
      edges: [
        { id: 'L_A_B_0', start: 'A', end: 'B', label: 'go', arrowTypeEnd: 'arrow_point', arrowTypeStart: 'none', pattern: 'normal' },
        { id: 'L_B_C_0', start: 'B', end: 'C', label: '', arrowTypeEnd: 'arrow_point', arrowTypeStart: 'none', pattern: 'dotted' },
        { id: 'L_B_D_0', start: 'B', end: 'D', label: '', arrowTypeEnd: 'arrow_point', arrowTypeStart: 'none', pattern: 'thick' },
        { id: 'L_D_E_0', start: 'D', end: 'E', label: '', arrowTypeEnd: 'none', arrowTypeStart: 'none', pattern: 'normal' },
      ],
    },
  };
}

/** The flowchart of `direction`, laid out by ELK and placed. */
async function laid(direction = 'LR') {
  const { data, classes } = parsed(direction);
  return placed(await new ELK().layout(graphOf(data, classes, direction, measure)));
}

test('a flowchart is told from the diagrams Mermaid still draws by its first word', () => {
  assert.ok(FLOWCHART.test('flowchart LR\n A-->B'));
  assert.ok(FLOWCHART.test('  graph TD; A-->B'));
  assert.ok(FLOWCHART.test('%% a comment\nflowchart TB\n A-->B'));
  assert.ok(!FLOWCHART.test('sequenceDiagram\n A->>B: hi'));
  assert.ok(!FLOWCHART.test('stateDiagram-v2\n [*] --> A'));
});

test('a node keeps its shape: a rectangle, a rounded rectangle, a stadium, a diamond or a circle', async () => {
  const { nodes } = await laid();
  const shape = (id) => nodes.find((node) => node.id === id)?.shape;
  assert.deepEqual(['A', 'B', 'C', 'D', 'E'].map(shape), ['rect', 'diamond', 'circle', 'stadium', 'round']);
  const drawn = flowchartSvg(await laid(), 'f1');
  assert.ok(drawn.querySelector('[data-id="B"] polygon'));
  assert.ok(drawn.querySelector('[data-id="C"] circle'));
  assert.equal(drawn.querySelector('[data-id="D"] rect')?.getAttribute('rx'), String(nodes.find((node) => node.id === 'D').height / 2));
  assert.equal(drawn.querySelector('[data-id="A"] rect')?.getAttribute('rx'), null, 'a rectangle is square');
});

test('each node is wide and tall enough for its label as measured, so that no label is cut', async () => {
  const { nodes } = await laid();
  for (const node of nodes) {
    const text = measure(node.lines);
    assert.ok(node.width >= text.width + 32, `${node.id} is wide enough`);
    assert.ok(node.height >= text.height + 20, `${node.id} is tall enough`);
  }
  // A rhombus holds its label's box where the box's corners lie inside it.
  const diamond = sizeOf('diamond', { width: 100, height: 18 });
  assert.ok((100 + 32) / diamond.width + (Math.max(36, 18 + 20)) / diamond.height <= 1.0001);
  assert.deepEqual(lines('one<br/>two'), ['one', 'two']);
});

test('each edge runs in horizontal and vertical segments that turn with rounded elbows', async () => {
  const { edges } = await laid();
  assert.equal(edges.length, 4);
  for (const edge of edges) {
    for (let index = 1; index < edge.points.length; index += 1) {
      const [a, b] = [edge.points[index - 1], edge.points[index]];
      assert.ok(Math.abs(a.x - b.x) < 0.5 || Math.abs(a.y - b.y) < 0.5, `${edge.id} turns only at right angles`);
    }
  }
  const path = pathOf([{ x: 0, y: 0 }, { x: 40, y: 0 }, { x: 40, y: 40 }], 6);
  assert.equal(path, 'M0 0 L34 0 A6 6 0 0 1 40 6 L40 40');
});

test('an edge ends in an arrowhead where its source gives one, a dotted edge is dashed and a thick one thicker', async () => {
  const drawn = flowchartSvg(await laid(), 'f2');
  const edge = (id) => /** @type {Element} */ ([...drawn.querySelectorAll('path.flow-edge')][['L_A_B_0', 'L_B_C_0', 'L_B_D_0', 'L_D_E_0'].indexOf(id)]);
  assert.equal(edge('L_A_B_0').getAttribute('marker-end'), 'url(#f2-arrow)');
  assert.equal(edge('L_D_E_0').getAttribute('marker-end'), null, 'an open link has no arrowhead');
  assert.match(edge('L_B_C_0').getAttribute('class') ?? '', /flow-edge--dotted/);
  assert.match(edge('L_B_D_0').getAttribute('class') ?? '', /flow-edge--thick/);
  const css = readFileSync(new URL('../../yard-page/yard.css', import.meta.url), 'utf8');
  assert.match(css, /\.flow-edge--dotted \{ stroke-dasharray/);
  assert.match(css, /\.flow-edge--thick \{ stroke-width: 2\.5px/);
});

test('an edge’s label lies in a pill beside its edge', async () => {
  const layout = await laid();
  const drawn = flowchartSvg(layout, 'f3');
  const pill = drawn.querySelector('.flow-pill');
  assert.equal(pill?.textContent, 'go');
  const rect = pill?.querySelector('rect');
  assert.equal(rect?.getAttribute('rx'), String(Number(rect?.getAttribute('height')) / 2), 'rounded at its ends');
  const label = layout.edges.find((edge) => edge.id === 'L_A_B_0')?.label;
  assert.ok(label && label.width >= measure(['go']).width, 'wide enough for its text');
});

test('a subgraph is a panel titled in small muted monospace capitals, holding its nodes', async () => {
  const { groups, nodes } = await laid();
  const [group] = groups;
  assert.equal(group.title, 'THE GROUP');
  assert.ok(group.width >= measure(['THE GROUP']).width + 32, 'at least as wide as its title');
  const inside = nodes.find((node) => node.id === 'C');
  assert.ok(inside.x >= group.x && inside.x + inside.width <= group.x + group.width, 'its node lies within it across');
  assert.ok(inside.y >= group.y + 20 && inside.y + inside.height <= group.y + group.height, 'and below its title');
  const drawn = flowchartSvg(await laid(), 'f4');
  assert.equal(drawn.querySelector('.flow-group .flow-group-title')?.textContent, 'THE GROUP');
  const css = readFileSync(new URL('../../yard-page/yard.css', import.meta.url), 'utf8');
  assert.match(css, /\.flow \.flow-group-title \{ fill: var\(--muted\); font-family: var\(--mono\); font-size: 11px;/);
});

test('the flowchart keeps its direction', async () => {
  const across = await laid('LR');
  const at = (layout, id) => layout.nodes.find((node) => node.id === id);
  assert.ok(at(across, 'A').x < at(across, 'B').x, 'left to right');
  const down = await laid('TB');
  assert.ok(at(down, 'A').y < at(down, 'B').y, 'top to bottom');
  const back = await laid('RL');
  assert.ok(at(back, 'A').x > at(back, 'B').x, 'right to left');
  const up = await laid('BT');
  assert.ok(at(up, 'A').y > at(up, 'B').y, 'bottom to top');
});

test('a node’s own style or classDef keeps its fill, its stroke and its text color', async () => {
  assert.deepEqual(styleOf({ cssClasses: 'default hot', cssStyles: ['stroke:#00f'] }, parsed().classes), { fill: '#fdd', stroke: '#00f', color: '#600' });
  assert.deepEqual(styleOf({ cssStyles: ['fill:url(javascript:x)', 'stroke-width:4px'] }, {}), {}, 'only a plain color is kept');
  const drawn = flowchartSvg(await laid(), 'f5');
  assert.equal(drawn.querySelector('[data-id="A"] rect')?.getAttribute('style'), 'fill:#eef');
  assert.equal(drawn.querySelector('[data-id="E"] rect')?.getAttribute('style'), 'fill:#fdd;stroke:#c00');
  assert.equal(drawn.querySelector('[data-id="E"] text')?.getAttribute('style'), 'fill:#600');
});

test('each node is a flat card with a hairline border on the sidebar’s ground, its label in the page’s face at 13 pixels in ink', () => {
  const css = readFileSync(new URL('../../yard-page/yard.css', import.meta.url), 'utf8');
  assert.match(css, /\.flow-node :is\(rect, polygon, circle\) \{ fill: var\(--side\); stroke: var\(--rail\); stroke-width: 1px; \}/);
  assert.match(css, /\.flow text \{ font-family: var\(--font\); font-size: 13px; \}/);
  assert.match(css, /\.flow-label \{ fill: var\(--ink\); \}/);
  assert.ok(!/filter|drop-shadow/.test(css.match(/\.flow-node[^}]*\}/g)?.join('') ?? ''), 'flat: no shadow');
});

test('a subgraph is at least as wide as its title, however narrow its nodes', async () => {
  const data = {
    nodes: [
      { id: 'S', label: 'A title much longer than the one node it holds', shape: 'rect', isGroup: true },
      { id: 'A', label: 'a', shape: 'squareRect', parentId: 'S', cssStyles: [], cssClasses: '' },
    ],
    edges: [],
  };
  const { groups } = placed(await new ELK().layout(graphOf(data, {}, 'LR', measure)));
  assert.ok(groups[0].width >= measure(['A TITLE MUCH LONGER THAN THE ONE NODE IT HOLDS']).width + 32);
});

test('each edge and its label name the nodes they join, for the hover layer', async () => {
  const drawn = flowchartSvg(await laid(), 'f6');
  const edge = drawn.querySelector('path.flow-edge');
  assert.equal(edge?.getAttribute('data-from'), 'A');
  assert.equal(edge?.getAttribute('data-to'), 'B');
  assert.equal(drawn.querySelector('.flow-pill')?.getAttribute('data-to'), 'B');
});

test('an edge within a subgraph runs between its nodes where they are drawn, not where the subgraph’s frame would put it', async () => {
  const data = {
    nodes: [
      { id: 'X', label: 'outside', shape: 'squareRect', cssStyles: [], cssClasses: '' },
      { id: 'S', label: 'Group', shape: 'rect', isGroup: true },
      { id: 'A', label: 'first', shape: 'squareRect', parentId: 'S', cssStyles: [], cssClasses: '' },
      { id: 'B', label: 'second', shape: 'squareRect', parentId: 'S', cssStyles: [], cssClasses: '' },
    ],
    edges: [
      { id: 'L_X_A_0', start: 'X', end: 'A', label: '', arrowTypeEnd: 'arrow_point', arrowTypeStart: 'none', pattern: 'normal' },
      { id: 'L_A_B_0', start: 'A', end: 'B', label: '', arrowTypeEnd: 'arrow_point', arrowTypeStart: 'none', pattern: 'normal' },
    ],
  };
  const layout = placed(await new ELK().layout(graphOf(data, {}, 'LR', measure)));
  const [a, b] = ['A', 'B'].map((id) => layout.nodes.find((node) => node.id === id));
  const inner = layout.edges.find((edge) => edge.id === 'L_A_B_0');
  const [start, end] = [inner.points[0], inner.points.at(-1)];
  assert.ok(Math.abs(start.x - (a.x + a.width)) < 1 && start.y >= a.y && start.y <= a.y + a.height, 'it leaves the first node');
  assert.ok(Math.abs(end.x - b.x) < 1 && end.y >= b.y && end.y <= b.y + b.height, 'and reaches the second');
});
