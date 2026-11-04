// A `spec-change` view's diagram as the page lays it out: ELK's graph of nested panels and cards.
// Each Product, Feature Group and Feature shown is a panel that holds what belongs to it; each
// Requirement, criterion, Component, decision and folded Feature Group is a card, headed by its
// label (or its kind's name) and its change, with its name or statement beneath, wrapped to the
// width the page's face measures. A criterion stands beside its Requirement, joined to it by a
// plain line; `realizes` and decision subject edges are dashed, and decisions and Components lie
// in the last column, a decision's edges laid out from its subjects so that it lies beside them.
// Nothing here reaches the page or the network.

import { PAD } from './flowchart.js';

/** The most boxes drawn before the Feature Groups are folded. */
export const MOST_BOXES = 30;

/** The kinds that are panels holding what belongs to them, unless folded. */
const PANELS = new Set(['product', 'feature_group', 'feature']);

/** Each kind's name, for a card or a panel whose node has no label yet. */
export const KIND_NAMES = {
  product: 'Product',
  feature_group: 'Feature Group',
  feature: 'Feature',
  requirement: 'Requirement',
  criterion: 'Criterion',
  component: 'Component',
  constraint: 'Constraint',
  quality_attribute: 'Quality Attribute',
  decision: 'Decision',
};

/** The word that says how a node changed. */
export const CHANGE_WORDS = { added: 'NEW', changed: 'CHANGED', removed: 'REMOVED' };

/** A card's room: its padding, its change bar, its head's height and the gap beneath it, in pixels. */
export const CARD = { pad: 12, bar: 3, head: 16, gap: 6, extra: 17, min: 140 };

/** A panel's room: its head's height and its padding. */
export const PANEL = { head: 44, pad: 12 };

/** The widest a card's text runs before it wraps. */
const MOST_WIDTH = { requirement: 220, criterion: 220 };
const MOST_WIDTH_ELSE = 200;

/**
 * The boxes the diagram draws: every box, or, past [`MOST_BOXES`], each Feature Group as one box
 * counting its changed nodes, but for the one open.
 * @param {any} graph
 * @param {string | null} open the key of the Feature Group open
 */
export function boxes(graph, open) {
  const nodes = graph?.nodes ?? [];
  const decisions = graph?.decisions ?? [];
  const all = [...nodes, ...decisions];
  if (all.length <= MOST_BOXES) return { shown: all, groups: [], folded: false };
  const byKey = new Map(nodes.map((node) => [node.key, node]));
  const groupOf = (node) => {
    let at = node;
    while (at && at.kind !== 'feature_group') at = byKey.get(at.parent);
    return at?.key ?? null;
  };
  const groups = nodes
    .filter((node) => node.kind === 'feature_group')
    .map((group) => ({
      ...group,
      count: nodes.filter((node) => node.key !== group.key && groupOf(node) === group.key && node.change).length,
    }));
  const shown = all.filter((node) => {
    const group = node.kind === 'feature_group' ? null : groupOf(node);
    return !group || group === open;
  });
  const marked = shown.map((node) => groups.find((group) => group.key === node.key && group.key !== open) ?? node);
  return { shown: marked, groups, folded: true };
}

/**
 * `text` in lines no wider than `most` as `measure` finds them, broken between words; a word
 * wider than `most` takes a line of its own, never cut.
 * @param {string} text
 * @param {number} most
 * @param {(lines: string[], kind?: string) => { width: number }} measure
 * @param {string} [kind]
 */
export function wrap(text, most, measure, kind) {
  const lines = [];
  let line = '';
  for (const word of String(text ?? '').split(/\s+/).filter(Boolean)) {
    const longer = line ? `${line} ${word}` : word;
    if (line && measure([longer], kind).width > most) {
      lines.push(line);
      line = word;
    } else line = longer;
  }
  if (line) lines.push(line);
  return lines;
}

/** Whether `node` is a Feature Group folded into one box. @param {any} node */
export function isFolded(node) {
  return node.kind === 'feature_group' && typeof node.count === 'number';
}

/**
 * What a box says beneath its name, in muted text: a folded Feature Group's count of its changed
 * nodes, and a Feature of a report recorded before the words were kept counting its Requirements
 * and criteria.
 * @param {any} node
 */
export function extras(node) {
  if (isFolded(node)) return [`${node.count} changed`, 'Open ›'];
  if (node.kind !== 'feature' || !node.delta) return [];
  /** @type {Record<'requirements' | 'criteria', [string, string]>} */
  const nouns = { requirements: ['Requirement', 'Requirements'], criteria: ['criterion', 'criteria'] };
  const count = (/** @type {number} */ n, /** @type {[string, string]} */ [one, many]) => `${n} ${n === 1 ? one : many}`;
  const lines = [`${count(node.requirements ?? 0, nouns.requirements)}, ${count(node.criteria ?? 0, nouns.criteria)}`];
  const delta = [];
  for (const kind of /** @type {const} */ (['requirements', 'criteria'])) {
    for (const change of ['added', 'changed', 'removed']) {
      const changed = node.delta[kind]?.[change];
      if (changed) delta.push(`${count(changed, nouns[kind])} ${change}`);
    }
  }
  if (delta.length) lines.push(delta.join(', '));
  return lines;
}

/**
 * The head of a box: its label, or its kind's name, and the word of its change.
 * @param {any} node
 */
export function headOf(node) {
  return {
    label: node.label || KIND_NAMES[/** @type {keyof typeof KIND_NAMES} */ (node.kind)] || String(node.kind ?? ''),
    word: CHANGE_WORDS[/** @type {keyof typeof CHANGE_WORDS} */ (node.change)] ?? null,
  };
}

/**
 * A card's ELK node: its size, and what the page draws in it.
 * @param {any} node
 * @param {string} id
 * @param {(lines: string[], kind?: string) => { width: number, height: number }} measure
 */
function cardOf(node, id, measure) {
  const { label, word } = headOf(node);
  const most = MOST_WIDTH[/** @type {keyof typeof MOST_WIDTH} */ (node.kind)] ?? MOST_WIDTH_ELSE;
  const lines = wrap(node.name ?? '', most, measure);
  const more = extras(node);
  const head = 12 + measure([label], 'spec-label').width + (word ? 16 + measure([word], 'spec-word').width : 0);
  const body = lines.length ? measure(lines).width : 0;
  const extra = more.length ? measure(more, 'spec-extra').width : 0;
  const inner = Math.max(head, body, extra);
  return {
    id,
    width: Math.ceil(Math.max(CARD.min, inner + 2 * CARD.pad + CARD.bar)),
    height: Math.ceil(2 * CARD.pad + CARD.head + (lines.length ? CARD.gap + lines.length * PAD.line : 0) + more.length * CARD.extra),
    card: {
      key: node.key,
      kind: node.kind,
      label,
      word,
      change: node.change ?? null,
      lines,
      extras: more,
      // Only a node that changed has an entry in the diff to bring into view; a folded Feature
      // Group opens.
      chosen: Boolean(node.change) || isFolded(node),
    },
  };
}

/**
 * A panel's ELK node: at least as wide as its head, holding what belongs to it.
 * @param {any} node
 * @param {string} id
 * @param {number} depth how many panels hold it
 * @param {(lines: string[], kind?: string) => { width: number, height: number }} measure
 */
function panelOf(node, id, depth, measure) {
  const { label, word } = headOf(node);
  const head = 12 + measure([label], 'spec-label').width + 8 + measure([node.name ?? ''], 'spec-panel-name').width + (word ? 16 + measure([word], 'spec-word').width : 0);
  return {
    id,
    group: true,
    panel: { key: node.key, kind: node.kind, label, name: node.name ?? '', word, change: node.change ?? null, depth, chosen: Boolean(node.change) },
    layoutOptions: {
      'elk.padding': `[top=${PANEL.head},left=${PANEL.pad},bottom=${PANEL.pad},right=${PANEL.pad}]`,
      'elk.nodeSize.constraints': 'MINIMUM_SIZE',
      'elk.nodeSize.minimum': `(${Math.ceil(head + 2 * PANEL.pad + CARD.bar)},${PANEL.head + PANEL.pad})`,
    },
    children: /** @type {any[]} */ ([]),
  };
}

/**
 * ELK's graph of a `spec-change` view's diagram, with the Feature Group `open` opened.
 * @param {any} graph the report's graph
 * @param {string | null} open
 * @param {(lines: string[], kind?: string) => { width: number, height: number }} measure a text's
 *   size, as the class `kind` sets it (a card's name where none is given)
 */
export function specGraph(graph, open, measure) {
  const { shown } = boxes(graph, open);
  const byKey = new Map(shown.map((node, index) => [node.key, { node, id: `n${index}` }]));
  const panel = (/** @type {any} */ node) => PANELS.has(node.kind) && !isFolded(node);
  /** The panel that holds `node`: the nearest panel shown above it. @param {any} node */
  const holder = (node) => {
    let at = byKey.get(node.parent)?.node;
    while (at && !panel(at)) at = byKey.get(at.parent)?.node;
    return at ?? null;
  };
  const depthOf = (/** @type {any} */ node) => {
    let depth = 0;
    for (let at = holder(node); at; at = holder(at)) depth += 1;
    return depth;
  };
  const root = {
    id: 'root',
    layoutOptions: {
      'elk.algorithm': 'layered',
      'elk.direction': 'RIGHT',
      'elk.edgeRouting': 'ORTHOGONAL',
      'elk.hierarchyHandling': 'INCLUDE_CHILDREN',
      'elk.layered.considerModelOrder.strategy': 'NODES_AND_EDGES',
      'elk.layered.crossingMinimization.forceNodeModelOrder': 'true',
      'elk.layered.nodePlacement.strategy': 'NETWORK_SIMPLEX',
      'elk.spacing.nodeNode': '12',
      'elk.layered.spacing.nodeNodeBetweenLayers': '40',
      'elk.spacing.edgeNode': '12',
      'elk.spacing.edgeEdge': '8',
      'elk.spacing.edgeLabel': '4',
      'elk.aspectRatio': '0.1',
      // Every edge's points in the diagram's own frame, whichever panel holds its ends.
      'elk.json.edgeCoords': 'ROOT',
    },
    children: /** @type {any[]} */ ([]),
    edges: /** @type {any[]} */ ([]),
  };
  /** @type {Map<string, any>} */
  const elkNodes = new Map();
  for (const { node, id } of byKey.values()) {
    const made = panel(node) ? panelOf(node, id, depthOf(node), measure) : cardOf(node, id, measure);
    // A decision and a Component lie in the last column, beside the panels they relate to.
    if (!holder(node) && !panel(node) && (node.kind === 'decision' || node.kind === 'component')) {
      made.layoutOptions = { ...made.layoutOptions, 'elk.layered.layering.layerConstraint': 'LAST' };
    }
    elkNodes.set(node.key, made);
  }
  for (const { node } of byKey.values()) {
    const held = holder(node);
    (held ? elkNodes.get(held.key).children : root.children).push(elkNodes.get(node.key));
  }
  let count = 0;
  // A box beneath a card, as a criterion beneath its Requirement, stands beside it, joined.
  for (const { node, id } of byKey.values()) {
    const parent = byKey.get(node.parent);
    if (!parent || panel(parent.node)) continue;
    root.edges.push({ id: `e${(count += 1)}`, sources: [parent.id], targets: [id], from: parent.id, to: id, pattern: 'tree', labels: [] });
  }
  for (const edge of graph?.edges ?? []) {
    const [from, to] = [byKey.get(edge.from), byKey.get(edge.to)];
    if (!from || !to) continue;
    const word = edge.kind === 'realizes' ? 'realizes' : edge.role ?? 'subject';
    const size = measure([word], 'flow-pill-label');
    // A decision's edge is laid out from its subject, so that the decision lies beside it; the
    // arrowhead still points at the subject.
    const subject = edge.kind !== 'realizes';
    root.edges.push({
      id: `e${(count += 1)}`,
      sources: [subject ? to.id : from.id],
      targets: [subject ? from.id : to.id],
      from: from.id,
      to: to.id,
      pattern: 'dotted',
      arrowEnd: !subject,
      arrowStart: subject,
      labels: [{ text: word, lines: [word], width: Math.ceil(size.width + 16), height: Math.ceil(size.height + 4) }],
    });
  }
  return root;
}

/** How far right of its Requirement the trunk of its criteria's lines runs, in pixels. */
export const TRUNK = 16;

/**
 * The placed diagram with the lines from each Requirement to its criteria drawn as one bracket:
 * out from the Requirement's head line, along one trunk, and into each criterion's head line.
 * @param {ReturnType<typeof import('./flowchart.js').placed>} layout
 */
export function trunks(layout) {
  const byId = new Map(layout.nodes.map((node) => [node.id, node]));
  const middle = (/** @type {any} */ node) => node.y + CARD.pad + CARD.head / 2;
  const edges = layout.edges.map((edge) => {
    const [from, to] = [byId.get(edge.from), byId.get(edge.to)];
    if (edge.pattern !== 'tree' || !from || !to || to.x <= from.x + from.width) return edge;
    const right = from.x + from.width;
    const trunk = Math.min(right + TRUNK, (right + to.x) / 2);
    const [out, into] = [middle(from), middle(to)];
    const points = Math.abs(out - into) < 0.5
      ? [{ x: right, y: out }, { x: to.x, y: into }]
      : [{ x: right, y: out }, { x: trunk, y: out }, { x: trunk, y: into }, { x: to.x, y: into }];
    return { ...edge, points };
  });
  return { ...layout, edges };
}
