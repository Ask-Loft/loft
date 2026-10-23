// The `loft` views: spec change, check findings, trace coverage, decision and publish state.

import { forget } from './dom.js';

import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';

import { add, h } from '../../yard-page/lib/dom.js';
import { decisionView } from '../../yard-page/views/loft/decision.js';
import { findingsView } from '../../yard-page/views/loft/findings.js';
import { loftView, named } from '../../yard-page/views/loft/index.js';
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

const feature = { key: 'f', kind: 'feature', label: 'F-3', name: 'Share a list', parent: 'g', change: 'changed' };
/** A Feature as a report recorded before the words were kept gives it. */
const counting = {
  ...feature,
  requirements: 4,
  criteria: 9,
  delta: { requirements: { added: 1, changed: 0, removed: 0 }, criteria: { added: 2, changed: 1, removed: 0 } },
};
const requirement = {
  key: 'r',
  kind: 'requirement',
  label: 'RQ-7',
  name: 'While a list is shared, the shelf shall refuse edits made through the link.',
  parent: 'f',
  change: null,
};
const criterion = { key: 'c1', kind: 'criterion', label: 'AC-21', name: 'An edit made through the link is refused, and the link says so.', parent: 'r', change: 'changed' };
const group = { key: 'g', kind: 'feature_group', label: 'FG-2', name: 'Sharing', parent: 'p', change: null };
const product = { key: 'p', kind: 'product', label: 'P-1', name: 'Reading List', parent: null, change: null };

test('a spec change diagram shows each changed node with the Feature Group and the Product above it', () => {
  const text = source({ nodes: [product, group, feature], decisions: [], edges: [] }, null);
  assert.match(text, /n0\["P-1 Reading List"\]:::context/);
  assert.match(text, /n1\["FG-2 Sharing"\]:::context/);
  assert.match(text, /n0 --> n1/);
  assert.match(text, /n1 --> n2/);
});

test('a spec change diagram colours each box by how it changed, in the colours of the scheme it is drawn in', () => {
  const colours = {
    ink: '#000001',
    muted: '#000002',
    rail: '#000003',
    surface2: '#000004',
    done: '#000005',
    doneSoft: '#000006',
    wait: '#000007',
    waitSoft: '#000008',
    danger: '#000009',
    dangerSoft: '#00000a',
  };
  const text = source({ nodes: [product, group, feature], decisions: [], edges: [] }, null, colours);
  assert.match(text, /classDef added fill:#000006,stroke:#000005,color:#000001/);
  assert.match(text, /classDef changed fill:#000008,stroke:#000007,color:#000001/);
  assert.match(text, /classDef removed fill:#00000a,stroke:#000009,color:#000001,stroke-dasharray:4 3/);
  assert.match(text, /classDef context fill:#000004,stroke:#000003,color:#000002/);
  assert.doesNotMatch(text, /#e3f4e8|#fdf0d8|#fbe4e2|#f1f3f5/);
});

test('a changed Requirement is drawn beneath its Feature holding its statement, and a changed criterion beneath its Requirement holding its name', () => {
  const text = source({ nodes: [product, group, feature, { ...requirement, change: 'added' }, criterion], decisions: [], edges: [] }, null);
  assert.match(text, /n3\("RQ-7 While a list is shared, the shelf shall<br\/>refuse edits made through the link\.<br\/>added"\):::added/);
  assert.match(text, /n4\(\["AC-21 An edit made through the link is<br\/>refused, and the link says so\.<br\/>changed"\]\):::changed/);
  assert.match(text, /n2 --> n3/);
  assert.match(text, /n3 --> n4/);
});

test('a Requirement above a changed criterion is drawn only for context, and a Feature box gives no count', () => {
  const text = source({ nodes: [product, group, feature, requirement, criterion], decisions: [], edges: [] }, null);
  assert.match(text, /n3\("RQ-7 [^"]*"\):::context/);
  assert.doesNotMatch(boxText(requirement), /added|changed|removed/);
  assert.equal(boxText(feature), 'F-3 Share a list<br/>changed');
});

test('a report recorded without the words counts a Feature box’s Requirements and criteria, and those added, changed or removed', () => {
  assert.equal(boxText(counting), 'F-3 Share a list<br/>changed<br/>4 Requirements, 9 criteria<br/>1 Requirement added, 2 criteria added, 1 criterion changed');
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

test('opening a Feature Group draws its changed Features, Requirements and criteria in place of its box, and opening another closes the first', () => {
  const nodes = [product];
  for (let g = 0; g < 4; g += 1) {
    nodes.push({ key: `g${g}`, kind: 'feature_group', label: `FG-${g}`, name: `Group ${g}`, parent: 'p', change: null });
    for (let f = 0; f < 9; f += 1) nodes.push({ ...feature, key: `g${g}f${f}`, parent: `g${g}` });
    nodes.push({ ...requirement, key: `g${g}r`, parent: `g${g}f0` }, { ...criterion, key: `g${g}c`, parent: `g${g}r` });
  }
  const one = boxes({ nodes, decisions: [], edges: [] }, 'g1');
  assert.equal(one.shown.filter((node) => node.kind === 'feature').length, 9);
  assert.deepEqual(
    one.shown.filter((node) => node.kind === 'requirement' || node.kind === 'criterion').map((node) => node.key),
    ['g1r', 'g1c'],
  );
  assert.ok(one.shown.every((node) => node.kind !== 'feature' || node.parent === 'g1'));
  const other = boxes({ nodes, decisions: [], edges: [] }, 'g2');
  assert.ok(other.shown.every((node) => node.kind !== 'feature' || node.parent === 'g2'));
});

test('a report recorded without the words lists every changed node by its kind, its label and its name in a tree that starts folded', () => {
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

/** A spec-change view of a report whose diff holds `diff`. */
function diffView(diff) {
  const report = {
    status: { base: { revision: 'develop', commit: '0123456789abcdef', added: [], changed: [], removed: [] } },
    graph: { nodes: [product, group, feature], decisions: [], edges: [], diff },
  };
  return loftView({ view: 'spec-change', report, recorded: RECORDED, base: 'develop', reasons: [] });
}

test('the diff beneath the diagram lists every changed node by its kind, its label and its name, and starts folded', () => {
  const view = diffView([
    { key: 'c1', kind: 'criterion', label: 'AC-21', name: 'A new one', change: 'added', words: { after: 'A new one' } },
    { key: 'd', kind: 'decision', label: null, name: 'Keep links short', change: 'removed', words: { before: 'Keep links short' } },
  ]);
  const diff = view.querySelector('.diff');
  assert.equal(diff.open, false);
  assert.match(diff.querySelector('summary').textContent, /2 nodes and decisions changed/);
  const items = [...diff.querySelectorAll('li')];
  assert.match(items[0].textContent, /^added criterion\s+AC-21\s*A new one/);
  assert.match(items[1].textContent, /^removed decision\s+Keep links short/);
});

test('an added node shows its words marked as added, and a removed node its words as they were, marked as removed', () => {
  const view = diffView([
    { key: 'c1', kind: 'criterion', label: null, name: 'A new one', change: 'added', words: { after: 'A new one' } },
    { key: 'c2', kind: 'criterion', label: 'AC-3', name: 'An old one', change: 'removed', words: { before: 'An old one' } },
  ]);
  const [added, removed] = view.querySelectorAll('.diff .words');
  assert.equal(added.querySelector('ins').textContent, 'A new one');
  assert.equal(added.querySelector('del'), null);
  assert.equal(removed.querySelector('del').textContent, 'An old one');
  assert.equal(removed.querySelector('ins'), null);
});

test('a changed node shows its words once, each word removed marked removed and each word added marked added', () => {
  const view = diffView([
    {
      key: 'c1',
      kind: 'criterion',
      label: 'AC-21',
      name: 'An edit made through the link is refused, and the link says so.',
      change: 'changed',
      words: { before: 'An edit made through the link is refused.', after: 'An edit made through the link is refused, and the link says so.' },
    },
  ]);
  const words = view.querySelector('.diff .words');
  assert.equal(words.querySelector('del').textContent, 'refused.');
  assert.equal(words.querySelector('ins').textContent, 'refused, and the link says so.');
  assert.match(words.textContent, /^An edit made through the link is /);
  assert.equal(view.querySelector('.words-same'), null);
});

test('a changed node whose words did not change says that only its other fields changed', () => {
  const statement = 'While a list is shared, the shelf shall refuse edits made through the link.';
  const view = diffView([{ key: 'r', kind: 'requirement', label: 'RQ-7', name: statement, change: 'changed', words: { before: statement, after: statement } }]);
  assert.equal(view.querySelector('.diff .words').textContent, statement);
  assert.equal(view.querySelector('.diff ins, .diff del'), null);
  assert.match(view.querySelector('.words-same').textContent, /Only its other fields changed/);
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
  assert.equal(view.querySelector('.trace-feature .accepted-toggle').textContent, '1 accepted');
  assert.match(view.querySelector('.trace-feature tbody.accepted .criterion-tests').textContent, /tests\/a\.rs:12one/);
  const reconciled = [...view.querySelectorAll('details.fold:not(.trace-list)')].find((fold) => /findings of loft reconcile/.test(fold.textContent));
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

/** A trace report over two Features, with code spans in its names, a link to review and a finding. */
function traced() {
  return {
    base: 'develop',
    features: [
      { key: 'f', label: 'F-3', name: '`loft share`', nameHtml: '<code>loft share</code>', criteria: [{ key: 'a', label: 'AC-1', name: 'One' }, { key: 'b', label: 'AC-2', name: 'Two' }] },
      { key: 'g', label: 'F-4', name: 'Keep a list', criteria: [{ key: 'e', label: 'AC-3', name: 'Three' }] },
    ],
    reconcile: { tests: [{ key: 'a', file: 'tests/a.rs', name: 'one' }, { key: 'b', file: 'tests/b.rs', name: 'two' }], findings: [{ key: 'b', rule: 'TR-002', message: 'not yet reviewed' }, { key: 'e', rule: 'TR-001', message: 'no link' }] },
  };
}

test('a trace view opens folded to its bar, its counts, its Features and its base', () => {
  const view = drawn(traceView(traced(), {}, {}));
  const list = view.querySelector('details.trace-list');
  assert.equal(list.open, false);
  assert.equal(list.querySelector('summary').textContent, '3 criteria under 2 Features');
  for (const figure of ['.coverage', '.coverage-words', '.counts', '.base']) assert.ok(!view.querySelector(figure).closest('details'), figure);
});

test('folded, a trace view lists no criterion and no finding', () => {
  const view = drawn(traceView(traced(), {}, {}));
  const list = view.querySelector('details.trace-list');
  assert.equal(list.open, false);
  assert.ok([...view.querySelectorAll('.criterion, .findings li, .trace-feature')].every((item) => list.contains(item)));
  assert.equal(view.querySelectorAll('.criterion').length, 3);
});

test('a trace view the person opened stays open when the page draws it again, and a folded one stays folded', () => {
  const first = drawn(traceView(traced(), {}, { key: 'trace-1' })).querySelector('details.trace-list');
  first.open = true;
  first.dispatchEvent(new Event('toggle'));
  assert.equal(drawn(traceView(traced(), {}, { key: 'trace-1' })).querySelector('details.trace-list').open, true);
  assert.equal(drawn(traceView(traced(), {}, { key: 'trace-2' })).querySelector('details.trace-list').open, false);
  first.open = false;
  first.dispatchEvent(new Event('toggle'));
  assert.equal(drawn(traceView(traced(), {}, { key: 'trace-1' })).querySelector('details.trace-list').open, false);
});

test('a node’s name is drawn from the HTML loft rendered from its Markdown, or as its text without it', () => {
  const rendered = drawn(named({ label: 'AC-1', name: 'Written to `<ours>`.', nameHtml: 'Written to <code>&lt;ours&gt;</code>.' }));
  assert.equal(rendered.querySelector('code').textContent, '<ours>');
  assert.equal(rendered.textContent, 'AC-1 Written to <ours>.');
  const plain = drawn(named({ label: 'AC-2', name: 'A <b>bold</b> `name`' }));
  assert.ok(!plain.querySelector('b, code'));
  assert.equal(plain.textContent, 'AC-2 A <b>bold</b> `name`');
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
