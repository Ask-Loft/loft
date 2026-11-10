// The line beside a stage of what its agents consulted: in the timeline's list and the stage's
// head, its consultations of the specification, its reads of code and whether the specification
// was consulted before the code was first edited; in the Gantt chart's column of names, the same
// in brief, its words for a reader and a pointer alike.

import { h } from '../lib/dom.js';
import { line } from '../model/consultation.js';

/**
 * The line of `stage`'s sums, or none where its agents consulted and edited no file.
 * @param {import('../model/consultation.js').Scope | undefined} stage
 * @param {{ brief?: boolean }} [options]
 */
export function consultLine(stage, options = {}) {
  const shown = line(stage);
  if (!shown) return null;
  const mark = shown.order?.mark;
  return h(
    'span',
    {
      class: ['consult-line', shown.order ? `consult-line--${shown.order.kind}` : null, options.brief ? 'consult-line--brief' : null],
      title: `Consulted: ${shown.said}`,
    },
    options.brief
      ? [h('span', { 'aria-hidden': 'true' }, [`${shown.consulted}·${shown.code}`, mark ? ` ${mark}` : null]), h('span', { class: 'sr-only' }, `Consulted: ${shown.said}`)]
      : shown.text,
  );
}
