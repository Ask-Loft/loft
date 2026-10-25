// A `spec-change` view: a diagram of the changed Products, Feature Groups, Features,
// Requirements, criteria, Components and decisions, each box saying how it changed, with the
// boxes above each for context and the `realizes` and subject edges between them; beneath it, a
// folded diff of the words of every changed node. A diagram of more than 30 boxes draws each
// Feature Group as one box that counts its changed nodes, and opens one Feature Group at a time.
// A report recorded before the words were kept has no diff: its Features count their
// Requirements and criteria, and a folded tree lists its changed nodes.

import { colours as schemeColours, draw } from '../../lib/diagrams.js';
import { h } from '../../lib/dom.js';
import { count } from '../../lib/format.js';
import * as kept from '../../lib/kept.js';
import { compareWords } from '../../model/words.js';
import { changeItem, leftOut } from './index.js';

/** The most boxes drawn before the Feature Groups are folded. */
export const MOST_BOXES = 30;

/** The most characters of a line of a box's name. */
const LINE = 44;

/**
 * `text` in lines of at most [`LINE`] characters, broken between words.
 * @param {string} text
 */
export function wrapped(text) {
  const lines = [];
  let line = '';
  for (const word of String(text ?? '').split(/\s+/).filter(Boolean)) {
    if (line && line.length + 1 + word.length > LINE) {
      lines.push(line);
      line = word;
    } else line = line ? `${line} ${word}` : word;
  }
  if (line) lines.push(line);
  return lines;
}

/** @param {string} text */
function quoted(text) {
  return String(text ?? '')
    .replaceAll('"', '#quot;')
    .replaceAll('<', '#lt;')
    .replaceAll('>', '#gt;')
    .replaceAll('`', "'");
}

/**
 * What a box says: its label and its name (a Requirement's statement, a criterion's name), and
 * how it changed; a Feature of a report recorded before the words were kept also counts its
 * Requirements and criteria.
 * @param {any} node
 */
export function boxText(node) {
  const lines = wrapped([node.label, node.name].filter(Boolean).join(' '));
  if (node.change) lines.push(node.change);
  if (node.kind === 'feature' && node.delta) {
    /** @type {Record<'requirements' | 'criteria', [string, string]>} */
    const nouns = { requirements: ['Requirement', 'Requirements'], criteria: ['criterion', 'criteria'] };
    lines.push(`${count(node.requirements ?? 0, ...nouns.requirements)}, ${count(node.criteria ?? 0, ...nouns.criteria)}`);
    const delta = [];
    for (const kind of /** @type {const} */ (['requirements', 'criteria'])) {
      for (const change of ['added', 'changed', 'removed']) {
        const changed = node.delta[kind]?.[change];
        if (changed) delta.push(`${count(changed, ...nouns[kind])} ${change}`);
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

/** The shape of each kind of box that is not a plain one. @type {Record<string, [string, string]>} */
const SHAPES = {
  decision: ['{{"', '"}}'],
  component: ['[("', '")]'],
  requirement: ['("', '")'],
  criterion: ['(["', '"])'],
};

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
    const shape = SHAPES[node.kind] ?? ['["', '"]'];
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
 * @param {{ key?: string }} [context]
 */
export function specChangeView(report, body, context = {}) {
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
  const unchanged = graph.diff ? !graph.diff.length : !changes.length && !decisions.length;
  if (unchanged) {
    return [h('p', { class: 'clean' }, `The specification has not changed since ${base.revision ?? 'the base revision'}.`), compared];
  }
  // The Feature Group the person opened, kept by the block, or by this drawing without one.
  const place = context.key ? `${context.key}/group` : null;
  /** @type {string | null} */
  let chosen = null;
  const opened = () => (place ? kept.get(place, null) : chosen);
  /** @param {string | null} key */
  const choose = (key) => (place ? kept.set(place, key, null) : (chosen = key));
  const target = h('div', { class: 'diagram spec-diagram' });
  const holder = h('div', { class: 'spec-change' }, compared, target);
  const redraw = () => {
    const open = opened();
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
                    choose(group.key === open ? null : group.key);
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
  const fold = { path: context.key ? `${context.key}/changes` : null, open: false };
  const diff = graph.diff;
  holder.append(
    diff
      ? kept.details(
          fold,
          { class: 'fold change-tree diff' },
          h('summary', {}, `${diff.length} nodes and decisions changed`),
          h('ul', {}, diff.map(diffItem)),
          leftOut(report, 'graph.diff'),
        )
      : kept.details(
          fold,
          { class: 'fold change-tree' },
          h('summary', {}, `${changes.length + decisions.length} nodes and decisions changed`),
          h('ul', {}, [...changes, ...decisions].map(changeItem)),
          ['added', 'changed', 'removed'].map((change) => leftOut(report, `status.base.${change}`)),
        ),
  );
  return holder;
}

/**
 * A node of the diff: how it changed, its kind, its label and its name, then its words: a changed
 * node's once, with the words removed and the words added marked in place; an added node's as
 * they are, the word beside them saying they were added; a removed node's struck through.
 * @param {{ change: string, kind: string, label?: string | null, name?: string,
 *   words?: { before?: string, after?: string } }} entry
 */
export function diffItem(entry) {
  const { before, after } = entry.words ?? {};
  /** @type {import('../../model/words.js').Run[]} */
  const runs =
    before === undefined
      ? [{ text: after ?? '', mark: 'same' }]
      : compareWords(before, after ?? '');
  const item = changeItem(entry);
  item.append(
    h(
      'div',
      { class: 'diff-words' },
      runs.map(({ text, mark }) => (mark === 'added' ? h('ins', {}, text) : mark === 'removed' ? h('del', {}, text) : text)),
    ),
  );
  if (before !== undefined && before === after) item.append(h('p', { class: 'diff-same' }, 'Only its other fields changed.'));
  return item;
}
