// A `loft` block's view, drawn from the report `loft` recorded in it, with where and when it
// was recorded.

import { h, inline } from '../../lib/dom.js';
import { when } from '../../lib/format.js';
import { decisionView } from './decision.js';
import { findingsView } from './findings.js';
import { publishView } from './publish.js';
import { specChangeView } from './spec-change.js';
import { traceView } from './trace.js';

const VIEWS = {
  'spec-change': specChangeView,
  findings: findingsView,
  trace: traceView,
  decision: decisionView,
  publish: publishView,
};

/**
 * @param {any} body the `loft` block's body
 * @param {{ branch?: string, key?: string }} [context]
 */
export function loftView(body, context = {}) {
  const draw = VIEWS[body.view];
  const recorded = body.recorded ?? {};
  return h(
    'div',
    { class: ['loft-view', `loft-view--${body.view}`] },
    draw ? draw(body.report ?? {}, body, context) : h('p', {}, `A ${body.view} view, which this page does not draw.`),
    h(
      'p',
      { class: 'recorded' },
      'Recorded ',
      recorded.commit ? h('code', {}, String(recorded.commit).slice(0, 10)) : 'before the first commit',
      recorded.branch ? ` on ${recorded.branch}` : null,
      recorded.dirty ? ', with changes not committed,' : null,
      ` by loft ${recorded.loft ?? ''}, `,
      h('time', { datetime: recorded.at }, when(recorded.at)),
    ),
  );
}

/**
 * A word with a colour, never a colour alone.
 * @param {string} word
 * @param {string} tone
 */
export function chip(word, tone) {
  return h('span', { class: ['chip', `chip--${tone}`] }, word);
}

/**
 * A label as a tag of its own, tinted by its node's kind, which its prefix names.
 * @param {string} label
 */
export function tag(label) {
  return h('span', { class: ['label', `label--${label.split('-')[0].toLowerCase()}`] }, label);
}

/**
 * A node's name: as the HTML `loft` rendered from its Markdown, where it gave one, or else as
 * its text.
 * @param {{ name?: string, nameHtml?: string }} node
 */
export function nameOf(node) {
  return node.nameHtml ? inline(node.nameHtml) : node.name ?? '';
}

/**
 * A node by its label, where it has one, and its name.
 * @param {{ label?: string | null, name?: string, nameHtml?: string }} node
 */
export function named(node) {
  return [node.label ? tag(node.label) : null, ' ', nameOf(node)];
}

/** The tone of each change a node can have gone through. @type {Record<string, string>} */
export const CHANGE_TONES = { added: 'success', changed: 'warning', removed: 'danger' };

/**
 * A node in a list of changes: how it changed, its kind and its name.
 * @param {{ change: string, kind: string, label?: string | null, name?: string }} node
 */
export function changeItem(node) {
  return h('li', {}, chip(node.change, CHANGE_TONES[node.change]), ' ', h('span', { class: 'kind' }, node.kind), ' ', named(node));
}

/**
 * How many of a report's list it left out, where it left some out.
 * @param {any} report
 * @param {string} path
 */
export function leftOut(report, path) {
  const count = report?.leftOut?.[path];
  return count ? h('p', { class: 'left-out' }, `${count} more not recorded in this view.`) : null;
}
