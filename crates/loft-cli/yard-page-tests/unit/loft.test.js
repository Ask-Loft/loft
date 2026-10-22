// The `loft` views: spec change, check findings, trace coverage, decision and publish state.

import { forget } from './dom.js';

import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';

import { add, h } from '../../yard-page/lib/dom.js';
import { decisionView } from '../../yard-page/views/loft/decision.js';
import { findingsView } from '../../yard-page/views/loft/findings.js';
import { loftView } from '../../yard-page/views/loft/index.js';
import { publishView } from '../../yard-page/views/loft/publish.js';
import { MOST_BOXES, boxes, boxText, source } from '../../yard-page/views/loft/spec-change.js';
import { linkState, traceView } from '../../yard-page/views/loft/trace.js';

beforeEach(forget);

/** Draws `children` into one element. */
function drawn(children) {
  const at = h('div');
  add(at, children);
  return at;
}

const RECORDED = { commit: 'abcdef0123456789', branch: 'feat/shelf', dirty: false, loft: '2.2.1', at: '2026-01-01T10:00:00Z' };

const feature = {
  key: 'f',
  kind: 'feature',
  label: 'F-3',
  name: 'Share a list',
  parent: 'g',
  change: 'changed',
  requirements: 4,
  criteria: 9,
  delta: { requirements: { added: 1, changed: 0, removed: 0 }, criteria: { added: 2, changed: 1, removed: 0 } },
};
const group = { key: 'g', kind: 'feature_group', label: 'FG-2', name: 'Sharing', parent: 'p', change: null };
const product = { key: 'p', kind: 'product', label: 'P-1', name: 'Reading List', parent: null, change: null };

test('a spec change diagram shows each changed node with the Feature Group and the Product above it', () => {
  const text = source({ nodes: [product, group, feature], decisions: [], edges: [] }, null);
  assert.match(text, /n0\["P-1 Reading List"\]:::context/);
  assert.match(text, /n1\["FG-2 Sharing"\]:::context/);
  assert.match(text, /n0 --> n1/);
  assert.match(text, /n1 --> n2/);
});

test('a Feature box counts its Requirements and criteria, and those added, changed or removed', () => {
  assert.equal(boxText(feature), 'F-3 Share a list<br/>changed<br/>4 Requirements, 9 criteria<br/>1 Requirements added, 2 criteria added, 1 criteria changed');
});

test('each box says how its node changed in a word, and a box drawn for context says neither', () => {
  assert.match(boxText({ ...product, change: 'added' }), /added/);
  assert.equal(boxText(product), 'P-1 Reading List');
});

test('the diagram draws the realizes and decision subject edges between the nodes it shows', () => {
  const component = { key: 'c', kind: 'component', label: 'CMP-3', name: 'Link service', parent: null, change: 'changed' };
  const decision = { key: 'd', kind: 'decision', label: null, name: 'Keep links short', change: 'added' };
  const text = source(
    {
      nodes: [product, group, feature, component],
      decisions: [decision],
      edges: [
        { from: 'f', to: 'c', kind: 'realizes' },
        { from: 'd', to: 'c', kind: 'subject', role: 'outcome' },
      ],
    },
    null,
  );
  assert.match(text, /n2 -\. realizes \.-> n3/);
  assert.match(text, /n4 -\. outcome \.-> n3/);
});

test('a diagram of more than 30 boxes draws each Feature Group as one box that counts its changed nodes', () => {
  const nodes = [product];
  for (let g = 0; g < 4; g += 1) {
    nodes.push({ key: `g${g}`, kind: 'feature_group', label: `FG-${g}`, name: `Group ${g}`, parent: 'p', change: null });
    for (let f = 0; f < 9; f += 1) nodes.push({ ...feature, key: `g${g}f${f}`, parent: `g${g}` });
  }
  assert.ok(nodes.length > MOST_BOXES);
  const folded = boxes({ nodes, decisions: [], edges: [] }, null);
  assert.equal(folded.folded, true);
  assert.equal(folded.shown.length, 5);
  assert.equal(folded.groups.find((each) => each.key === 'g1').count, 9);
  assert.match(boxText(folded.shown[2]), /9 changed/);
});

test('opening a Feature Group draws its changed Features in place of its box, and opening another closes the first', () => {
  const nodes = [product];
  for (let g = 0; g < 4; g += 1) {
    nodes.push({ key: `g${g}`, kind: 'feature_group', label: `FG-${g}`, name: `Group ${g}`, parent: 'p', change: null });
    for (let f = 0; f < 9; f += 1) nodes.push({ ...feature, key: `g${g}f${f}`, parent: `g${g}` });
  }
  const one = boxes({ nodes, decisions: [], edges: [] }, 'g1');
  assert.equal(one.shown.filter((node) => node.kind === 'feature').length, 9);
  assert.ok(one.shown.every((node) => node.kind !== 'feature' || node.parent === 'g1'));
  const other = boxes({ nodes, decisions: [], edges: [] }, 'g2');
  assert.ok(other.shown.every((node) => node.kind !== 'feature' || node.parent === 'g2'));
});

test('the tree beneath the diagram lists every changed node by its kind, its label and its name, and starts folded', () => {
  const report = {
    status: { base: { revision: 'develop', commit: '0123456789abcdef', added: [{ kind: 'criterion', label: null, name: 'A new one' }], changed: [], removed: [] } },
    graph: { nodes: [feature], decisions: [], edges: [] },
  };
  const view = loftView({ view: 'spec-change', report, recorded: RECORDED, base: 'develop', reasons: [] });
  const tree = view.querySelector('.change-tree');
  assert.equal(tree.open, false);
  assert.match(tree.querySelector('li').textContent, /added criterion\s+A new one/);
  assert.match(view.querySelector('.base').textContent, /Compared with develop at 0123456789/);
  assert.match(view.querySelector('.recorded').textContent, /Recorded abcdef0123 on feat\/shelf/);
});

test('a spec change view of no change says that the specification has not changed since the base revision', () => {
  const report = { status: { base: { revision: 'develop', added: [], changed: [], removed: [] } }, graph: { nodes: [], decisions: [], edges: [] } };
  const view = loftView({ view: 'spec-change', report, recorded: RECORDED, reasons: [] });
  assert.match(view.querySelector('.clean').textContent, /has not changed since develop/);
});

const finding = (more) => ({ rule: 'LQ-005', severity: 'info', message: 'The text may hold more than one statement.', path: 'spec/a.yaml', line: 7, change: 'introduced', subjects: [{ key: 'n1' }], ...more });

test('a findings view counts the errors and the findings at each severity, each with its rule, severity, message and place', () => {
  const errors = [{ kind: 'parse', message: 'The file is not YAML.', sites: [{ path: 'spec/b.yaml', line: 2 }] }];
  const check = { errors, findings: [finding({}), finding({ severity: 'warning', rule: 'LQ-002' })], resolved: [], baseline: { revision: 'develop' } };
  const view = drawn(findingsView({ check, nodes: {} }, { reasons: [] }));
  assert.deepEqual([...view.querySelector('.counts').children].map((chip) => chip.textContent), ['1 error', '1 warning', '1 info']);
  const [error, first] = view.querySelectorAll('.finding');
  assert.match(error.textContent, /parse error The file is not YAML\./);
  assert.equal(error.querySelector('.site code').textContent, 'spec/b.yaml:2');
  assert.match(first.textContent, /LQ-005 info introduced/);
  assert.equal(first.querySelector('.site code').textContent, 'spec/a.yaml:7');
  assert.match(view.querySelector('.base').textContent, /against develop/);
});

test('a finding names its node by label and name, is marked introduced or there before, and shows the reason it was left', () => {
  const report = { check: { errors: [], findings: [finding({ change: 'preexisting' })], resolved: [finding({ rule: 'LQ-009' })] }, nodes: { n1: { label: 'AC-4', name: 'A link expires' } } };
  const view = drawn(findingsView(report, { reasons: [{ rule: 'LQ-005', key: 'n1', reason: 'one rule, said once' }] }));
  assert.match(view.querySelector('.node').textContent, /AC-4\s+A link expires/);
  assert.match(view.querySelector('.finding').textContent, /there before/);
  assert.match(view.querySelector('.finding .reason').textContent, /one rule, said once/);
  assert.match(view.querySelector('.resolved summary').textContent, /1 resolved by the branch/);
});

test('a check with no error and no finding says that the specification is clean', () => {
  const view = drawn(findingsView({ check: { errors: [], findings: [], resolved: [] } }, { reasons: [] }));
  assert.match(view.querySelector('.clean').textContent, /clean/);
});

test('each criterion of a trace view is marked accepted, not yet reviewed, stale, unverified with its reason, or not linked', () => {
  const tests = new Map([['a', [{ file: 'tests/a.rs', name: 'a', locations: [{ firstLine: 3 }] }]], ['b', [{}]], ['c', [{}]]]);
  const rules = new Map([['b', ['TR-002']], ['c', ['TR-003']], ['e', ['TR-001']]]);
  assert.equal(linkState({ key: 'a' }, rules, tests), 'accepted');
  assert.equal(linkState({ key: 'b' }, rules, tests), 'unreviewed');
  assert.equal(linkState({ key: 'c' }, rules, tests), 'stale');
  assert.equal(linkState({ key: 'd', unverified: 'by hand' }, rules, tests), 'unverified');
  assert.equal(linkState({ key: 'e' }, rules, tests), 'unlinked');
});

test('a trace view bars the criteria linked and accepted, counts each state where there is more than one, and folds the accepted', () => {
  const report = {
    base: 'develop',
    features: [{ key: 'f', label: 'F-3', name: 'Share a list', criteria: [{ key: 'a', label: 'AC-1', name: 'One' }, { key: 'b', label: 'AC-2', name: 'Two' }, { key: 'e', name: 'Three' }] }],
    reconcile: { tests: [{ key: 'a', file: 'tests/a.rs', name: 'one', locations: [{ firstLine: 12 }] }, { key: 'b', file: 'tests/b.rs', name: 'two' }], findings: [{ key: 'b', rule: 'TR-002', message: 'not yet reviewed' }, { key: 'e', rule: 'TR-001', message: 'no link' }] },
  };
  const view = drawn(traceView(report));
  assert.match(view.querySelector('.coverage-words').textContent, /2 of 3 .* linked to tests, 1 of those accepted/);
  assert.deepEqual([...view.querySelector('.counts').children].map((chip) => chip.textContent), ['1 accepted', '1 not yet reviewed', '1 not linked']);
  assert.match(view.querySelector('.base').textContent, /Compared with develop/);
  const accepted = view.querySelector('.trace-feature details');
  assert.equal(accepted.querySelector('summary').textContent, '1 accepted');
  assert.match(accepted.querySelector('.tests').textContent, /tests\/a\.rs:12 one/);
  const reconciled = [...view.querySelectorAll('details.fold')].find((fold) => /findings of loft reconcile/.test(fold.textContent));
  assert.match(reconciled.querySelector('summary').textContent, /2 findings of loft reconcile/);
  assert.deepEqual([...reconciled.querySelectorAll('li')].map((item) => item.textContent), ['TR-002 not yet reviewed', 'TR-001 no link']);
});

test('where every criterion is in one state, no count is given beside the bar', () => {
  const report = {
    features: [{ key: 'f', label: 'F-3', name: 'Share a list', criteria: [{ key: 'a', label: 'AC-1', name: 'One' }] }],
    reconcile: { tests: [{ key: 'a', file: 'tests/a.rs', name: 'one' }], findings: [] },
  };
  const view = drawn(traceView(report));
  assert.ok(view.querySelector('.coverage'));
  assert.ok(!view.querySelector('.counts'));
});

test('a decision view shows the decision, its options with the selected one marked, and a table of its assessments', () => {
  const report = {
    decision: {
      key: 'd', label: null, title: 'Store a shelf in one file', context: 'Why', slug: 's',
      state: { kind: 'accepted', outcome: 'One file' },
      drivers: [{ key: 'r1', kind: 'local', label: 'Speed', statement: 'Fast reads' }],
      options: [{ key: 'o1', title: 'One file', selected: true, description: 'All in one' }, { key: 'o2', title: 'Many files' }],
      assessments: [{ option: 'o1', driver: 'r1', rating: 'supports', rationale: 'one read' }, { option: 'o2', driver: 'r1', rating: 'conflicts', rationale: 'many reads' }],
      consequences: [{ key: 'c1', kind: 'risk', statement: 'A big file' }],
      subjects: [{ role: 'affected', node: { key: 'n', label: 'CMP-1', name: 'Shelf store' } }],
    },
  };
  const view = drawn(decisionView(report));
  assert.match(view.querySelector('.decision-view-head').textContent, /Store a shelf in one file\s*accepted/);
  assert.match(view.querySelector('.unpublished').textContent, /no label/);
  assert.ok(view.querySelector('.decision-option.is-selected').textContent.startsWith('One file'));
  const cells = [...view.querySelectorAll('.assessments tbody td')].map((cell) => cell.textContent);
  assert.deepEqual(cells, ['supportsone read', 'conflictsmany reads']);
  assert.equal(view.querySelector('.assessments thead th:nth-child(2)').textContent, 'Speed');
  const tones = [...view.querySelectorAll('.assessments td .chip')].map((chip) => chip.className);
  assert.equal(new Set(tones).size, 2, `each rating has its own colour: ${tones}`);
  assert.match([...view.querySelectorAll('details li')].map((item) => item.textContent).join('\n'), /Speed Fast reads/);
  assert.match(view.querySelector('.consequences').textContent, /risk A big file/);
  assert.match(view.querySelector('.subjects').textContent, /affected CMP-1\s+Shelf store/);
});

test('a decision that is not accepted shows its state, and a rejected one its rationale', () => {
  const view = drawn(decisionView({ decision: { title: 'T', label: 'ADR-2', state: { kind: 'rejected', rationale: 'Too slow' }, options: [], drivers: [] } }));
  assert.match(view.textContent, /rejected/);
  assert.match(view.querySelector('.rationale').textContent, /Too slow/);
});

test('a publish view shows the lock’s sequence, time and state root, and whether the tree matches it', () => {
  const lock = { publishSequence: 12, publishedAt: '2026-01-01T09:00:00Z', stateRoot: '226c06e356ba6918c65a', matches: true };
  const view = drawn(publishView({ status: { lock }, labels: { feature: 3, decision: 2 } }, { recorded: RECORDED }, { branch: 'feat/shelf' }));
  assert.match(view.querySelector('.lock').textContent, /sequence 12/);
  assert.equal(view.querySelector('.lock code').textContent, '226c06e356ba');
  assert.match(view.querySelector('.lock').textContent, /matches the lock/);
  assert.equal(view.querySelector('.labels').textContent, 'Carrying a label: 3 Features, 2 decisions.');
});

test('a tree that differs from the lock lists each node added, changed or removed since the publish', () => {
  const lock = { publishSequence: 12, publishedAt: '2026-01-01T09:00:00Z', stateRoot: 'x', matches: false };
  const base = { added: [{ kind: 'criterion', label: null, name: 'New' }], changed: [{ kind: 'feature', label: 'F-1', name: 'Old' }], removed: [] };
  const view = drawn(publishView({ status: { lock, base } }, { recorded: RECORDED }, {}));
  assert.match(view.querySelector('.lock').textContent, /differs from the lock/);
  assert.deepEqual([...view.querySelectorAll('.changes li')].map((item) => item.textContent.replace(/\s+/g, ' ').trim()), ['added criterion New', 'changed feature F-1 Old']);
});

test('a working tree with no lock says that the specification has not been published', () => {
  const view = drawn(publishView({ status: { lock: null } }, { recorded: RECORDED }, {}));
  assert.match(view.querySelector('.unpublished').textContent, /has not been published/);
});

test('a publish recorded after Yard Log closed says it was recorded on the base branch after the merge', () => {
  const view = drawn(publishView({ status: { lock: null } }, { recorded: { ...RECORDED, branch: 'develop' } }, { branch: 'feat/shelf' }));
  assert.match(view.querySelector('.after-merge').textContent, /Recorded on develop after the merge/);
});
