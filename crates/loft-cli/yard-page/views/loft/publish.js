// A `publish` view: the publish sequence and time the lock pins, the start of its state root,
// whether the tree matches it, each node added, changed or removed since the publish, and the
// labelled nodes and decisions counted by kind.

import { h } from '../../lib/dom.js';
import { when } from '../../lib/format.js';
import * as kept from '../../lib/kept.js';
import { changeItem, chip, leftOut } from './index.js';

const KIND_NAMES = {
  product: 'Products',
  feature_group: 'Feature Groups',
  feature: 'Features',
  requirement: 'Requirements',
  criterion: 'criteria',
  component: 'Components',
  constraint: 'constraints',
  quality_attribute: 'quality attributes',
  section: 'Sections',
  decision: 'decisions',
};

/**
 * @param {any} report
 * @param {any} body
 * @param {{ branch?: string, key?: string }} context
 */
export function publishView(report, body, context) {
  const status = report.status ?? {};
  const lock = status.lock;
  const after = context.branch && body.recorded?.branch && body.recorded.branch !== context.branch;
  const base = status.base ?? {};
  const changes = ['added', 'changed', 'removed'].flatMap((change) =>
    (base[change] ?? []).map((node) => ({ ...node, change })),
  );
  const labels = Object.entries(report.labels ?? {});
  return [
    after
      ? h('p', { class: 'after-merge' }, `Recorded on ${body.recorded.branch} after the merge, once Yard Log was closed.`)
      : null,
    lock
      ? h(
          'dl',
          { class: 'lock' },
          h('dt', {}, 'Publish'),
          h('dd', {}, `sequence ${lock.publishSequence}, `, h('time', { datetime: lock.publishedAt }, when(lock.publishedAt))),
          h('dt', {}, 'State root'),
          h('dd', {}, h('code', {}, String(lock.stateRoot).slice(0, 12))),
          h('dt', {}, 'The tree'),
          h('dd', {}, lock.matches ? chip('matches the lock', 'success') : chip('differs from the lock', 'warning')),
        )
      : h('p', { class: 'unpublished' }, 'The specification has not been published: the tree holds no lock.'),
    lock && !lock.matches && changes.length
      ? kept.details(
          { path: context.key ? `${context.key}/changes` : null, open: false },
          { class: 'fold' },
          h('summary', {}, `${changes.length} nodes added, changed or removed since the publish`),
          h('ul', { class: 'changes' }, changes.map(changeItem)),
          ['added', 'changed', 'removed'].map((change) => leftOut(report, `status.base.${change}`)),
        )
      : null,
    labels.length
      ? h(
          'p',
          { class: 'labels' },
          'Carrying a label: ',
          labels.map(([kind, count], index) => [index ? ', ' : '', `${count} ${KIND_NAMES[kind] ?? kind}`]),
          '.',
        )
      : null,
  ];
}

