// A `spec-change` view: a diagram of the changed Products, Feature Groups, Features, Components
// and decisions, each box saying how it changed, with the Feature Group and Product above each
// for context and the `realizes` and subject edges between them; beneath it, a folded tree of
// every changed node. A diagram of more than 30 boxes draws each Feature Group as one box that
// counts its changed nodes, and opens one Feature Group at a time.

import { colours as schemeColours, draw } from '../../lib/diagrams.js';
import { h } from '../../lib/dom.js';
import { changeItem, leftOut } from './index.js';

/** The most boxes drawn before the Feature Groups are folded. */
export const MOST_BOXES = 30;

/** The Feature Group open in each view, by the view's block key. */
const opened = new Map();

/** @param {string} text */
function quoted(text) {
  return String(text ?? '')
    .replaceAll('"', '#quot;')
    .replaceAll('<', '#lt;')
    .replaceAll('>', '#gt;')
    .replaceAll('`', "'");
}

/**
 * What a box says: its label and name, how it changed, and for a Feature its counts.
 * @param {any} node
 */
export function boxText(node) {
  const lines = [[node.label, node.name].filter(Boolean).join(' ')];
  if (node.change) lines.push(node.change);
  if (node.kind === 'feature') {
    lines.push(`${node.requirements ?? 0} Requirements, ${node.criteria ?? 0} criteria`);
    const delta = [];
    for (const [kind, counts] of [['Requirements', node.delta?.requirements], ['criteria', node.delta?.criteria]]) {
      for (const change of ['added', 'changed', 'removed']) {
        if (counts?.[change]) delta.push(`${counts[change]} ${kind} ${change}`);
      }
    }
    if (delta.length) lines.push(delta.join(', '));
  }
  if (node.count) lines.push(`${node.count} changed`);
  return lines.map(quoted).join('<br/>');
}

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
 * The diagram's Mermaid source, its boxes coloured by how their nodes changed.
 * @param {any} graph
 * @param {string | null} open
 * @param {Pick<import('../../lib/diagrams.js').Colours, 'ink' | 'muted' | 'rail' | 'surface2' | 'done' | 'doneSoft' | 'wait' | 'waitSoft' | 'danger' | 'dangerSoft'>} [c] the scheme's colours
 */
export function source(graph, open, c = schemeColours()) {
  const { shown } = boxes(graph, open);
  const ids = new Map(shown.map((node, index) => [node.key, `n${index}`]));
  const lines = ['flowchart TD'];
  for (const node of shown) {
    const shape = node.kind === 'decision' ? ['{{"', '"}}'] : node.kind === 'component' ? ['[("', '")]'] : ['["', '"]'];
    lines.push(`  ${ids.get(node.key)}${shape[0]}${boxText(node)}${shape[1]}:::${node.change ?? 'context'}`);
  }
  for (const node of shown) {
    if (node.parent && ids.has(node.parent)) lines.push(`  ${ids.get(node.parent)} --> ${ids.get(node.key)}`);
  }
  for (const edge of graph?.edges ?? []) {
    if (!ids.has(edge.from) || !ids.has(edge.to)) continue;
    const word = edge.kind === 'realizes' ? 'realizes' : edge.role ?? 'subject';
    lines.push(`  ${ids.get(edge.from)} -. ${word} .-> ${ids.get(edge.to)}`);
  }
  lines.push(
    `  classDef added fill:${c.doneSoft},stroke:${c.done},color:${c.ink}`,
    `  classDef changed fill:${c.waitSoft},stroke:${c.wait},color:${c.ink}`,
    `  classDef removed fill:${c.dangerSoft},stroke:${c.danger},color:${c.ink},stroke-dasharray:4 3`,
    `  classDef context fill:${c.surface2},stroke:${c.rail},color:${c.muted}`,
  );
  return lines.join('\n');
}

/**
 * @param {any} report
 * @param {any} body
 */
export function specChangeView(report, body) {
  const status = report.status ?? {};
  const base = status.base ?? {};
  const graph = report.graph ?? { nodes: [], decisions: [], edges: [] };
  const changes = ['added', 'changed', 'removed'].flatMap((change) =>
    (base[change] ?? []).map((node) => ({ ...node, change })),
  );
  const decisions = graph.decisions ?? [];
  const compared = h(
    'p',
    { class: 'base' },
    `Compared with ${base.revision ?? body.base ?? 'the base branch'}`,
    base.commit ? [' at ', h('code', {}, String(base.commit).slice(0, 10))] : null,
    '.',
  );
  if (!changes.length && !decisions.length) {
    return [h('p', { class: 'clean' }, `The specification has not changed since ${base.revision ?? 'the base revision'}.`), compared];
  }
  const key = body.recorded?.at ?? '';
  const target = h('div', { class: 'diagram spec-diagram' });
  const holder = h('div', { class: 'spec-change' }, compared, target);
  const redraw = () => {
    const open = opened.get(key) ?? null;
    const { groups, folded } = boxes(graph, open);
    void draw(target, (c) => source(graph, open, c));
    picker.replaceChildren(
      ...(folded
        ? [
            h('span', { class: 'picker-words' }, 'Open a Feature Group: '),
            ...groups.map((group) =>
              h(
                'button',
                {
                  type: 'button',
                  class: ['group-button', group.key === open ? 'is-open' : null],
                  'aria-pressed': group.key === open ? 'true' : 'false',
                  onclick: () => {
                    opened.set(key, group.key === open ? null : group.key);
                    redraw();
                  },
                },
                `${group.label ?? ''} ${group.name} (${group.count})`.trim(),
              ),
            ),
          ]
        : []),
    );
  };
  const picker = h('div', { class: 'group-picker' });
  holder.append(picker);
  redraw();
  holder.append(
    h(
      'details',
      { class: 'fold change-tree' },
      h('summary', {}, `${changes.length + decisions.length} nodes and decisions changed`),
      h(
        'ul',
        {},
        [...changes, ...decisions].map(changeItem),
      ),
      ['added', 'changed', 'removed'].map((change) => leftOut(report, `status.base.${change}`)),
    ),
  );
  return holder;
}
