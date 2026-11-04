// A Mermaid flowchart as the page lays it out and draws it itself: the nodes, edges and groups
// that Mermaid's parser gives, turned into a graph for ELK's layered layout, each node sized to
// its label as the page's face measures it; and ELK's answer turned back into what the page draws,
// every position in the flowchart's own frame, each edge a path of horizontal and vertical
// segments that turns with rounded elbows. Nothing here reaches the page or the network.

/** Mermaid's shapes, as the page draws them; any other shape is drawn as a rectangle. */
const SHAPES = {
  rect: 'rect',
  squareRect: 'rect',
  roundedRect: 'round',
  rounded: 'round',
  stadium: 'stadium',
  diamond: 'diamond',
  question: 'diamond',
  circle: 'circle',
  doublecircle: 'circle',
};

/** The room around a label inside its node, and the height of one line, in pixels. */
export const PAD = { x: 16, y: 10, line: 18 };

/** The room a group's title takes above its nodes, and beside them, in pixels. */
export const TITLE = { top: 36, side: 16 };

/** ELK's direction for each of Mermaid's. */
const DIRECTIONS = { TB: 'DOWN', TD: 'DOWN', BT: 'UP', LR: 'RIGHT', RL: 'LEFT' };

/**
 * A label's lines: Mermaid's line breaks kept, any other markup and its entities left as text.
 * @param {unknown} label
 */
export function lines(label) {
  return String(label ?? '')
    .split(/<br\s*\/?>/i)
    .map((line) => line.replace(/<[^>]*>/g, '').replace(/&nbsp;/g, ' ').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&amp;/g, '&').trim())
    .filter((line, index, all) => line || all.length === 1);
}

/**
 * The fill, stroke and text colour a node's own `classDef` and `style` give it, the `style` last.
 * @param {any} node
 * @param {Record<string, string[]>} classes
 * @returns {{ fill?: string, stroke?: string, color?: string }}
 */
export function styleOf(node, classes) {
  const declared = [
    ...String(node.cssClasses ?? '').split(/\s+/).flatMap((name) => classes[name] ?? []),
    ...(node.cssStyles ?? []),
  ];
  /** @type {Record<string, string>} */
  const style = {};
  for (const each of declared) {
    const [name, ...rest] = String(each).split(':');
    const value = rest.join(':').trim();
    if (['fill', 'stroke', 'color'].includes(name.trim()) && /^[#\w(),.%\s-]+$/.test(value)) style[name.trim()] = value;
  }
  return style;
}

/**
 * A node's size for its shape, about a label of `text` size.
 * @param {string} shape
 * @param {{ width: number, height: number }} text
 */
export function sizeOf(shape, text) {
  const width = text.width + 2 * PAD.x;
  const height = Math.max(36, text.height + 2 * PAD.y);
  switch (shape) {
    case 'diamond':
      // A rhombus 1.6 times as wide holds the label's box when it is 8/3 as tall.
      return { width: Math.ceil(width * 1.6), height: Math.ceil((height * 8) / 3) };
    case 'circle': {
      const diameter = Math.ceil(Math.hypot(width, height));
      return { width: diameter, height: diameter };
    }
    case 'stadium':
      return { width: Math.ceil(width + height / 2), height };
    default:
      return { width: Math.ceil(width), height: Math.ceil(height) };
  }
}

/**
 * ELK's graph of a flowchart, from what Mermaid's parser gives.
 * @param {{ nodes: any[], edges: any[] }} data `getData()` of Mermaid's flowchart database
 * @param {Record<string, string[]>} classes each `classDef`'s styles, by its name
 * @param {string} direction Mermaid's direction: TB, TD, BT, LR or RL
 * @param {(lines: string[], kind?: string) => { width: number, height: number }} measure a label's
 *   size, as the class `kind` sets it (a node's label where none is given)
 */
export function graphOf(data, classes, direction, measure) {
  /** @type {Map<string, any>} */
  const elkNodes = new Map();
  const root = {
    id: 'root',
    layoutOptions: {
      'elk.algorithm': 'layered',
      'elk.direction': DIRECTIONS[direction] ?? 'DOWN',
      'elk.edgeRouting': 'ORTHOGONAL',
      'elk.hierarchyHandling': 'INCLUDE_CHILDREN',
      'elk.layered.considerModelOrder.strategy': 'NODES_AND_EDGES',
      'elk.layered.nodePlacement.strategy': 'NETWORK_SIMPLEX',
      'elk.spacing.nodeNode': '32',
      'elk.layered.spacing.nodeNodeBetweenLayers': '48',
      'elk.spacing.edgeNode': '16',
      'elk.spacing.edgeEdge': '12',
      'elk.layered.spacing.edgeNodeBetweenLayers': '16',
      'elk.spacing.edgeLabel': '6',
      'elk.edgeLabels.inline': 'false',
      // Every edge's points in the flowchart's own frame, whichever subgraph holds its ends.
      'elk.json.edgeCoords': 'ROOT',
    },
    children: /** @type {any[]} */ ([]),
    edges: /** @type {any[]} */ ([]),
  };
  for (const node of data.nodes) {
    const text = lines(node.label);
    if (node.isGroup) {
      // A group's title is set in capitals, and the group is at least as wide as it.
      const title = text.join(' ').toUpperCase();
      const wide = measure([title], 'flow-group-title').width + 2 * TITLE.side;
      elkNodes.set(node.id, {
        id: node.id,
        title,
        group: true,
        layoutOptions: {
          'elk.padding': `[top=${TITLE.top},left=${TITLE.side},bottom=${TITLE.side},right=${TITLE.side}]`,
          'elk.nodeSize.constraints': 'MINIMUM_SIZE',
          'elk.nodeSize.minimum': `(${Math.ceil(wide)},${TITLE.top + TITLE.side})`,
        },
        children: [],
      });
      continue;
    }
    const shape = SHAPES[node.shape] ?? 'rect';
    const measured = measure(text);
    elkNodes.set(node.id, { id: node.id, shape, lines: text, text: measured, style: styleOf(node, classes), ...sizeOf(shape, measured) });
  }
  for (const node of data.nodes) {
    const parent = node.parentId && elkNodes.get(node.parentId);
    (parent ? parent.children : root.children).push(elkNodes.get(node.id));
  }
  for (const edge of data.edges) {
    if (!elkNodes.has(edge.start) || !elkNodes.has(edge.end)) continue;
    const text = lines(edge.label).filter(Boolean);
    const size = text.length ? measure(text) : null;
    root.edges.push({
      id: edge.id,
      sources: [edge.start],
      targets: [edge.end],
      pattern: edge.pattern === 'dotted' || edge.stroke === 'dotted' ? 'dotted' : edge.pattern === 'thick' || edge.stroke === 'thick' ? 'thick' : 'normal',
      arrowEnd: edge.arrowTypeEnd && edge.arrowTypeEnd !== 'none',
      arrowStart: edge.arrowTypeStart && edge.arrowTypeStart !== 'none',
      labels: size ? [{ text: text.join('\n'), lines: text, width: Math.ceil(size.width + 16), height: Math.ceil(size.height + 6) }] : [],
    });
  }
  return root;
}

/**
 * What the page draws from ELK's answer: every node, group, edge and edge label, placed in the
 * flowchart's own frame, which ELK gives each relative to the node that holds it.
 * @param {any} laid ELK's laid-out graph
 */
export function placed(laid) {
  /** @type {any[]} */
  const nodes = [];
  /** @type {any[]} */
  const groups = [];
  /** @type {any[]} */
  const edges = [];
  /**
   * @param {any} node
   * @param {number} dx
   * @param {number} dy
   */
  const visit = (node, dx, dy) => {
    for (const child of node.children ?? []) {
      const x = dx + (child.x ?? 0);
      const y = dy + (child.y ?? 0);
      const parent = node.id === 'root' ? null : node.id;
      if (child.group) groups.push({ id: child.id, parent, title: child.title, panel: child.panel, x, y, width: child.width, height: child.height });
      else nodes.push({ id: child.id, parent, shape: child.shape, lines: child.lines, text: child.text, style: child.style, card: child.card, x, y, width: child.width, height: child.height });
      visit(child, x, y);
    }
    for (const edge of node.edges ?? []) {
      const points = (edge.sections ?? []).flatMap((section) => [section.startPoint, ...(section.bendPoints ?? []), section.endPoint]).map((point) => ({ x: dx + point.x, y: dy + point.y }));
      const label = edge.labels?.[0];
      edges.push({
        id: edge.id,
        // An edge laid out against its direction keeps its own ends.
        from: edge.from ?? edge.sources?.[0],
        to: edge.to ?? edge.targets?.[0],
        points,
        pattern: edge.pattern,
        arrowEnd: Boolean(edge.arrowEnd),
        arrowStart: Boolean(edge.arrowStart),
        label: label ? { lines: label.lines, x: dx + label.x, y: dy + label.y, width: label.width, height: label.height } : null,
      });
    }
  };
  visit(laid, 0, 0);
  return { width: Math.ceil(laid.width ?? 0), height: Math.ceil(laid.height ?? 0), nodes, groups, edges };
}

/**
 * An SVG path through `points` in straight segments, each elbow turned with an arc of `radius`
 * or of half the shorter segment beside it, whichever is less.
 * @param {{ x: number, y: number }[]} points
 * @param {number} radius
 */
export function pathOf(points, radius) {
  if (!points.length) return '';
  const round = (/** @type {number} */ value) => Math.round(value * 10) / 10;
  const parts = [`M${round(points[0].x)} ${round(points[0].y)}`];
  for (let index = 1; index < points.length - 1; index += 1) {
    const [before, at, after] = [points[index - 1], points[index], points[index + 1]];
    const into = Math.hypot(at.x - before.x, at.y - before.y);
    const out = Math.hypot(after.x - at.x, after.y - at.y);
    const r = Math.min(radius, into / 2, out / 2);
    if (!r) {
      parts.push(`L${round(at.x)} ${round(at.y)}`);
      continue;
    }
    const start = { x: at.x - ((at.x - before.x) / into) * r, y: at.y - ((at.y - before.y) / into) * r };
    const end = { x: at.x + ((after.x - at.x) / out) * r, y: at.y + ((after.y - at.y) / out) * r };
    // Which way the path turns: clockwise where the cross product of its two segments is positive.
    const sweep = (at.x - before.x) * (after.y - at.y) - (at.y - before.y) * (after.x - at.x) > 0 ? 1 : 0;
    parts.push(`L${round(start.x)} ${round(start.y)}`, `A${round(r)} ${round(r)} 0 0 ${sweep} ${round(end.x)} ${round(end.y)}`);
  }
  const last = points[points.length - 1];
  parts.push(`L${round(last.x)} ${round(last.y)}`);
  return parts.join(' ');
}
