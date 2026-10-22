// A `loft` block's view, drawn from the report `loft` recorded in it, with where and when it
// was recorded.

import { h } from '../../lib/dom.js';
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
 * @param {{ branch?: string }} [context]
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
 * A node by its label, where it has one, and its name.
 * @param {{ label?: string | null, name?: string }} node
 */
export function named(node) {
  return [node.label ? h('span', { class: 'label' }, node.label) : null, ' ', node.name ?? ''];
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
