// The list of a `trace` view: each Feature folded to its count and a bar, its Requirements by
// their statements, and its criteria as a table; and the labels of every `loft` view.

import { forget } from './dom.js';

import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';

import { add, h } from '../../yard-page/lib/dom.js';
import { named } from '../../yard-page/views/loft/index.js';
import { traceView } from '../../yard-page/views/loft/trace.js';

beforeEach(forget);

/** Draws `children` into one element. */
function drawn(children) {
  const at = h('div');
  add(at, children);
  return at;
}

/**
 * A trace report of one Feature with two Requirements, as `loft` records it and the page server
 * renders its Markdown: one Requirement in an EARS form, one in plain text.
 */
function report() {
  return {
    base: 'develop',
    features: [
      {
        key: 'f',
        label: 'F-3',
        name: 'Share a list',
        requirements: [
          {
            key: 'r1',
            label: 'RQ-6',
            name: 'While a list is shared, the shelf shall refuse edits made through the link.',
            statement: {
              form: 'state_driven',
              condition: 'a list is shared',
              system: 'the shelf',
              shall: 'refuse edits made through the `link`.',
              shallHtml: 'refuse edits made through the <code>link</code>.',
            },
          },
          { key: 'r2', label: null, name: 'Lists stay private unless shared.' },
        ],
        criteria: [
          { key: 'a', label: 'AC-7', name: 'An edit made through the link is refused.', requirement: 'r1' },
          { key: 'b', label: 'AC-8', name: 'The link says so.', requirement: 'r1' },
          { key: 'c', label: null, name: 'A list starts private.', requirement: 'r2', unverified: 'Read by hand' },
          { key: 'd', label: 'AC-9', name: 'Nobody else sees it.', requirement: 'r2' },
          { key: 'e', label: 'AC-10', name: 'A shared list can be private again.', requirement: 'r2' },
        ],
      },
    ],
    reconcile: {
      tests: [
        { key: 'a', file: 'tests/share.rs', name: 'an_edit_is_refused', locations: [{ firstLine: 12 }] },
        { key: 'b', file: 'tests/share.rs', name: 'the_link_says_so', locations: [{ firstLine: 30 }] },
        { key: 'd', file: 'tests/private.rs', name: 'nobody_else_sees_it' },
      ],
      findings: [
        { key: 'b', rule: 'TR-002', message: 'not yet reviewed' },
        { key: 'e', rule: 'TR-001', message: 'no link' },
      ],
    },
  };
}

/** The view's Feature folds. */
const features = (view) => [...view.querySelectorAll('details.trace-feature')];

test('each Feature of a trace view starts folded to its label, its name, the number of its criteria linked and a bar', () => {
  const [feature] = features(drawn(traceView(report(), {}, {})));
  assert.equal(feature.open, false);
  const summary = feature.querySelector('summary');
  assert.equal(summary.querySelector('.label').textContent, 'F-3');
  assert.match(summary.textContent, /Share a list/);
  assert.equal(summary.querySelector('.feature-count').textContent, '4 of 5 linked');
  assert.equal(parseFloat(summary.querySelector('.coverage-linked').style.width), 80);
  assert.ok(summary.querySelector('.coverage').getAttribute('aria-label'));
});

test('a Feature the person opened stays open when the page draws it again', () => {
  const [first] = features(drawn(traceView(report(), {}, { key: 'trace-9' })));
  first.open = true;
  first.dispatchEvent(new Event('toggle'));
  assert.equal(features(drawn(traceView(report(), {}, { key: 'trace-9' })))[0].open, true);
  assert.equal(features(drawn(traceView(report(), {}, { key: 'trace-10' })))[0].open, false);
});

test('each Requirement shows its label and its statement with its EARS keywords set apart, above its criteria', () => {
  const groups = [...drawn(traceView(report(), {}, {})).querySelectorAll('tbody.requirement')];
  const head = groups[0].querySelector('.requirement-row');
  assert.equal(head.querySelector('.label').textContent, 'RQ-6');
  assert.equal(head.querySelector('.statement').textContent, 'While a list is shared, the shelf shall refuse edits made through the link.');
  assert.deepEqual([...head.querySelectorAll('.ears-word')].map((word) => word.textContent), ['While', 'shall']);
  assert.equal(head.querySelector('.statement code').textContent, 'link');
  assert.equal(groups[0].querySelector('tr'), head);
  assert.equal(groups[1].querySelector('.statement').textContent, 'Lists stay private unless shared.');
  assert.ok(!groups[1].querySelector('.ears-word'));
});

test('the criteria table gives each criterion’s label, name, state and tests, those that need a look first and the accepted behind one line', () => {
  const table = drawn(traceView(report(), {}, {})).querySelector('table.criteria-table');
  assert.deepEqual([...table.querySelectorAll('thead th')].map((cell) => cell.textContent), ['Label', 'Criterion', 'State', 'Tests']);
  const [first] = [...table.querySelectorAll('tbody.requirement')];
  const rows = [...first.querySelectorAll('tr.criterion')];
  assert.deepEqual(rows.map((row) => [...row.cells].map((cell) => cell.textContent)), [['AC-8', 'The link says so.', 'not yet reviewed', 'tests/share.rs:30the_link_says_so']]);
  const toggle = first.querySelector('button.accepted-toggle');
  assert.equal(toggle.textContent, '1 accepted');
  const accepted = toggle.closest('table').querySelectorAll('tbody.accepted')[0];
  assert.equal(accepted.hidden, true);
  assert.match(accepted.textContent, /AC-7.*An edit made through the link is refused\..*accepted.*tests\/share\.rs:12/);
  toggle.click();
  assert.equal(accepted.hidden, false);
  assert.equal(toggle.getAttribute('aria-expanded'), 'true');
});

test('a criterion with no label yet reads new where its label would be', () => {
  const row = [...drawn(traceView(report(), {}, {})).querySelectorAll('tr.criterion')].find((each) => /starts private/.test(each.textContent));
  const label = row.cells[0].querySelector('.label');
  assert.equal(label.textContent, 'new');
  assert.ok(label.classList.contains('label--new'));
  assert.equal(row.querySelector('.reason').textContent, 'Read by hand');
});

test('a trace view whose report gives no Requirement lists each Feature’s criteria in one table', () => {
  const flat = report();
  delete flat.features[0].requirements;
  for (const criterion of flat.features[0].criteria) delete criterion.requirement;
  const view = drawn(traceView(flat, {}, {}));
  assert.equal(view.querySelectorAll('table.criteria-table').length, 1);
  assert.ok(!view.querySelector('.requirement-row'));
  assert.equal(view.querySelectorAll('tr.criterion').length, 5);
});

test('a label is a tag of its own, tinted by its node’s kind', () => {
  const tags = ['F-3', 'RQ-6', 'AC-7', 'CMP-2'].map((label) => drawn(named({ label, name: 'A node' })).querySelector('.label'));
  assert.deepEqual(
    tags.map((each) => [...each.classList]),
    [
      ['label', 'label--f'],
      ['label', 'label--rq'],
      ['label', 'label--ac'],
      ['label', 'label--cmp'],
    ],
  );
  assert.equal(tags[0].nextSibling.textContent, ' ');
});
