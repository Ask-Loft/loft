// A `spec-change` view: a diagram of the changed Products, Feature Groups, Features,
// Requirements, criteria, Components and decisions, drawn by the page as nested panels and cards
// (`lib/spec-diagram.js`), each saying how it changed, with what holds each for context and the
// `realizes` and subject edges between them, under a legend that counts the changes; beneath it,
// a folded diff of the words of every changed node, which choosing a card unfolds at its entry. A diagram of more than 30 boxes draws each
// Feature Group as one box that counts its changed nodes, and opens one Feature Group at a time.
// A report recorded before the words were kept has no diff: its Features count their
// Requirements and criteria, and a folded tree lists its changed nodes.

import { draw } from '../../lib/diagrams.js';
import { h } from '../../lib/dom.js';
import * as kept from '../../lib/kept.js';
import { digest, drawSpecChange } from '../../lib/spec-diagram.js';
import { CHANGE_WORDS, boxes, isFolded } from '../../model/spec-graph.js';
import { compareWords } from '../../model/words.js';
import { changeItem, leftOut } from './index.js';

/**
 * The legend above the diagram: each change its boxes and panels show, keyed by its bar, with
 * how many show it.
 * @param {any[]} shown
 */
export function legend(shown) {
  const counts = ['added', 'changed', 'removed'].map((change) => [change, shown.filter((node) => node.change === change).length]);
  const held = counts.filter(([, n]) => n);
  if (!held.length) return null;
  return h(
    'p',
    { class: 'spec-legend' },
    held.map(([change, n]) =>
      h('span', { class: 'spec-legend-item' }, h('span', { class: `spec-key spec-key--${change}`, 'aria-hidden': 'true' }), `${n} ${CHANGE_WORDS[/** @type {keyof typeof CHANGE_WORDS} */ (change)].toLowerCase()}`),
    ),
  );
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
  const key = digest(JSON.stringify([graph.nodes, graph.decisions, graph.edges]));
  /** Brings the diff's entry of the node `chosen` into view, unfolding the diff, and marks it. @param {string} chosen */
  const show = (chosen) => {
    const entry = /** @type {HTMLElement | null} */ ([...holder.querySelectorAll('.change-tree li[data-key]')].find((each) => /** @type {HTMLElement} */ (each).dataset.key === chosen) ?? null);
    const fold = /** @type {HTMLDetailsElement | null} */ (holder.querySelector('details.change-tree'));
    if (fold) fold.open = true;
    for (const each of holder.querySelectorAll('.change-tree li.is-marked')) each.classList.remove('is-marked');
    if (!entry) return;
    entry.classList.add('is-marked');
    entry.scrollIntoView?.({ block: 'nearest', behavior: 'smooth' });
  };
  const legendAt = h('div', { class: 'spec-legend-at' });
  const holder = h('div', { class: 'spec-change' }, compared, legendAt, target);
  const redraw = () => {
    const open = opened();
    const { shown, groups, folded } = boxes(graph, open);
    legendAt.replaceChildren(...[legend(shown)].filter(Boolean));
    void draw(target, {
      key: `spec\n${key}\n${open ?? ''}`,
      draw: (id, expanded) => drawSpecChange(graph, open, id, expanded),
      choose: (chosen) => {
        const group = shown.find((node) => node.key === chosen && isFolded(node));
        if (group) {
          choose(group.key);
          redraw();
        } else show(chosen);
      },
    });
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
 * @param {{ key?: string, change: string, kind: string, label?: string | null, name?: string,
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
  if (entry.key) item.dataset.key = entry.key;
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
