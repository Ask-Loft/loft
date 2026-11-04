// The `loft` views: spec change, check findings, trace coverage, decision and publish state.

import { forget } from './dom.js';

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { beforeEach, test } from 'node:test';
import { gunzipSync } from 'node:zlib';

import { add, h } from '../../yard-page/lib/dom.js';
import { decisionView } from '../../yard-page/views/loft/decision.js';
import { findingsView } from '../../yard-page/views/loft/findings.js';
import { loftView, named } from '../../yard-page/views/loft/index.js';
import { publishView } from '../../yard-page/views/loft/publish.js';
import { CARD, MOST_BOXES, TRUNK, boxes, extras, headOf, specGraph, trunks, wrap } from '../../yard-page/model/spec-graph.js';
import { placed } from '../../yard-page/model/flowchart.js';
import { flowchartSvg } from '../../yard-page/views/flowchart-svg.js';
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

/** The ELK the page server serves, run as the page runs it. */
const ELK = (() => {
  const code = gunzipSync(readFileSync(new URL('../../yard-page/vendor/elk.bundled.js.gz', import.meta.url))).toString('utf8');
  const module = { exports: {} };
  new Function('module', 'exports', code)(module, module.exports);
  return /** @type {any} */ (module.exports);
})();
/** @type {any} */ (globalThis).ELK = ELK;

/** A text's size at 7 pixels a character and 18 a line. @param {string[]} text */
const measure = (text) => ({ width: Math.max(0, ...text.map((line) => line.length * 7)), height: text.length * 18 });

/** The diagram of `graph`, laid out by ELK and placed. */
async function laid(graph, open = null) {
  return placed(await new ELK().layout(specGraph({ decisions: [], edges: [], ...graph }, open, measure)));
}

/** The card or panel of the node `key` in `layout`. */
const at = (layout, key) => [...layout.nodes, ...layout.groups].find((each) => (each.card ?? each.panel)?.key === key);

/** Whether the box `a` lies within the box `b`. */
const within = (a, b) => a.x >= b.x && a.y >= b.y && a.x + a.width <= b.x + b.width && a.y + a.height <= b.y + b.height;

const css = readFileSync(new URL('../../yard-page/yard.css', import.meta.url), 'utf8');

test('a spec change diagram shows each changed node with the Feature Group and the Product above it', async () => {
  const layout = await laid({ nodes: [product, group, feature] });
  assert.ok(at(layout, 'p')?.panel, 'the Product is a panel');
  assert.ok(within(at(layout, 'g'), at(layout, 'p')), 'that holds its Feature Group');
  assert.ok(within(at(layout, 'f'), at(layout, 'g')), 'that holds its Feature');
});

test('each Product, Feature Group and Feature is a panel headed by its kind’s square, its label and its name', async () => {
  const drawn = flowchartSvg(await laid({ nodes: [product, group, feature] }), 'p1');
  const panels = [...drawn.querySelectorAll('.spec-panel')];
  assert.deepEqual(panels.map((panel) => panel.querySelector('.spec-kind')?.getAttribute('class')), [
    'spec-kind spec-kind--product',
    'spec-kind spec-kind--feature_group',
    'spec-kind spec-kind--feature',
  ]);
  assert.deepEqual(panels.map((panel) => [panel.querySelector('.spec-label')?.textContent, panel.querySelector('.spec-panel-name')?.textContent]), [
    ['P-1', 'Reading List'],
    ['FG-2', 'Sharing'],
    ['F-3', 'Share a list'],
  ]);
  assert.match(css, /\.spec-panel-frame \{ stroke: var\(--line\); stroke-width: 1px; \}/, 'framed by a hairline');
});

test('a changed Requirement is drawn inside its Feature’s panel holding its statement, and a changed criterion to its right holding its name', async () => {
  const second = { ...criterion, key: 'c2', label: 'AC-22', name: 'A second criterion of the same Requirement.' };
  const layout = await laid({ nodes: [product, group, feature, { ...requirement, change: 'added' }, criterion, second] });
  const [held, first, next] = [at(layout, 'r'), at(layout, 'c1'), at(layout, 'c2')];
  assert.ok(within(held, at(layout, 'f')));
  assert.equal(held.card.lines.join(' '), requirement.name);
  assert.equal(first.card.lines.join(' '), criterion.name);
  assert.ok(first.x >= held.x + held.width, 'to its right');
  assert.ok(Math.abs(first.x - next.x) < 1 && next.y >= first.y + first.height, 'its criteria stacked one beneath another');
  const joining = layout.edges.filter((edge) => edge.pattern === 'tree');
  assert.equal(joining.length, 2);
  assert.ok(joining.every((edge) => !edge.arrowEnd && !edge.arrowStart && !edge.label), 'a plain line, no arrowhead');
});

test('a Requirement above a changed criterion is drawn only for context, and a Feature box gives no count', async () => {
  const layout = await laid({ nodes: [product, group, feature, requirement, criterion] });
  assert.equal(at(layout, 'r').card.change, null);
  assert.equal(at(layout, 'r').card.word, null);
  assert.deepEqual(extras(feature), []);
  const drawn = flowchartSvg(layout, 'p2');
  const context = drawn.querySelector('.spec-card--context');
  assert.ok(context && !context.querySelector('.spec-bar, .spec-word'), 'no bar and no word');
  assert.match(css, /\.spec-card--context \.spec-frame \{ fill: var\(--side\); \}/);
  assert.match(css, /\.spec-card--context \.spec-name \{ fill: var\(--muted\); \}/);
});

test('a report recorded without the words counts a Feature box’s Requirements and criteria, and those added, changed or removed', () => {
  assert.deepEqual(extras(counting), ['4 Requirements, 9 criteria', '1 Requirement added, 2 criteria added, 1 criterion changed']);
});

test('each box says how its node changed in a word, and a box drawn for context says neither', async () => {
  assert.deepEqual(headOf({ ...product, change: 'added' }), { label: 'P-1', word: 'NEW' });
  assert.deepEqual(headOf({ ...criterion, change: 'removed' }), { label: 'AC-21', word: 'REMOVED' });
  assert.deepEqual(headOf(product), { label: 'P-1', word: null });
  const drawn = flowchartSvg(await laid({ nodes: [product, group, feature, requirement, criterion] }), 'p3');
  assert.equal(drawn.querySelector('.spec-card--changed .spec-word')?.textContent, 'CHANGED');
});

test('a box gives its node’s label in monospace, or its kind’s name where the node has none yet', () => {
  assert.equal(headOf({ ...criterion, label: null }).label, 'Criterion');
  assert.equal(headOf({ key: 'd', kind: 'decision', label: null, name: 'x' }).label, 'Decision');
  assert.match(css, /\.flow \.spec-label \{ font-family: var\(--mono\);/);
});

test('a box’s name is wrapped to its measured width, so that nothing is cut', async () => {
  const lines = wrap(requirement.name, 220, measure);
  assert.ok(lines.length > 1 && lines.every((line) => measure([line]).width <= 220));
  assert.equal(lines.join(' '), requirement.name);
  assert.deepEqual(wrap('a Supercalifragilisticexpialidocious b', 70, measure), ['a', 'Supercalifragilisticexpialidocious', 'b'], 'a word wider than the measure is never cut');
  const card = at(await laid({ nodes: [product, group, feature, { ...requirement, change: 'added' }] }), 'r');
  assert.ok(card.width >= measure(card.card.lines).width + 2 * CARD.pad + CARD.bar, 'wide enough');
  assert.ok(card.height >= 2 * CARD.pad + CARD.head + CARD.gap + card.card.lines.length * 18, 'tall enough');
});

test('an added, changed or removed box carries a bar at its left in its change’s colour, and a removed one is outlined dashed', async () => {
  const drawn = flowchartSvg(await laid({ nodes: [{ ...product, change: 'added' }, { ...group, change: 'removed' }, feature] }), 'p4');
  for (const change of ['added', 'changed', 'removed']) {
    const box = drawn.querySelector(`.spec-panel--${change}`);
    const bar = box?.querySelector('.spec-bar');
    assert.equal(bar?.getAttribute('x'), box?.querySelector('.spec-panel-frame')?.getAttribute('x'), `${change}: at its left`);
    assert.equal(bar?.getAttribute('width'), '3');
  }
  assert.match(css, /\.spec-card--removed \.spec-frame, \.spec-panel--removed \.spec-panel-frame \{ stroke: var\(--rail\); stroke-dasharray: 4 3; \}/);
  assert.match(css, /\.spec-bar \{ fill: var\(--change\); \}/);
});

test('the bars of change are the validated steps of green, blue and red in each scheme', () => {
  const [light, dark] = css.split('@media (prefers-color-scheme: dark)');
  for (const [scheme, colours] of [[light, ['#199e70', '#256abf', '#e34948']], [dark, ['#199e70', '#3987e5', '#e66767']]]) {
    const [added, changed, removed] = colours;
    assert.match(scheme, new RegExp(`--change-added: ${added};`));
    assert.match(scheme, new RegExp(`--change-changed: ${changed};`));
    assert.match(scheme, new RegExp(`--change-removed: ${removed};`));
  }
});

test('the diagram draws the realizes and decision subject edges between the nodes it shows', async () => {
  const component = { key: 'c', kind: 'component', label: 'CMP-3', name: 'Link service', parent: null, change: 'changed' };
  const decision = { key: 'd', kind: 'decision', label: null, name: 'Keep links short', change: 'added' };
  const layout = await laid({
    nodes: [product, group, feature, component],
    decisions: [decision],
    edges: [
      { from: 'f', to: 'c', kind: 'realizes' },
      { from: 'd', to: 'c', kind: 'subject', role: 'outcome' },
    ],
  });
  const id = (key) => at(layout, key).id;
  const realizes = layout.edges.find((edge) => edge.label?.lines[0] === 'realizes');
  const outcome = layout.edges.find((edge) => edge.label?.lines[0] === 'outcome');
  assert.deepEqual([realizes.from, realizes.to, realizes.pattern, realizes.arrowEnd], [id('f'), id('c'), 'dotted', true]);
  assert.deepEqual([outcome.from, outcome.to, outcome.pattern, outcome.arrowStart, outcome.arrowEnd], [id('d'), id('c'), 'dotted', true, false]);
  // The arrowhead is drawn at the subject: the path ends where it starts at the Component.
  const subject = at(layout, 'c');
  const [start] = outcome.points;
  assert.ok(Math.abs(start.x - subject.x) < 1 || Math.abs(start.x - (subject.x + subject.width)) < 1 || Math.abs(start.y - subject.y) < 1 || Math.abs(start.y - (subject.y + subject.height)) < 1, 'it leaves from the subject');
  const right = at(layout, 'p').x + at(layout, 'p').width;
  assert.ok(at(layout, 'd').x >= right && at(layout, 'c').x >= right, 'decisions and Components lie beside the panels, to their right');
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
  assert.deepEqual(extras(folded.shown[2]), ['9 changed', 'Open ›']);
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

test('an added node shows its words unmarked, and a removed node its words as they were, marked as removed', () => {
  const view = diffView([
    { key: 'c1', kind: 'criterion', label: null, name: 'A new one', change: 'added', words: { after: 'A new one' } },
    { key: 'c2', kind: 'criterion', label: 'AC-3', name: 'An old one', change: 'removed', words: { before: 'An old one' } },
  ]);
  const [added, removed] = view.querySelectorAll('.diff .diff-words');
  assert.equal(added.textContent, 'A new one');
  assert.equal(added.querySelector('ins, del'), null);
  assert.equal(removed.querySelector('del').textContent, 'An old one');
  assert.equal(removed.querySelector('ins'), null);
});

test('a node of the diff says nothing more than its change, its kind, its label, its name and its words', () => {
  const view = diffView([
    { key: 'c1', kind: 'criterion', label: null, name: 'A new one', change: 'added', words: { after: 'A new one' } },
    { key: 'c2', kind: 'criterion', label: 'AC-4', name: 'Two', change: 'changed', words: { before: 'One', after: 'Two' } },
  ]);
  for (const item of view.querySelectorAll('.diff li')) assert.doesNotMatch(item.textContent, /null|undefined/);
  assert.equal(view.querySelector('.diff .words'), null, 'the page’s muted grid of words is another thing');
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
  const words = view.querySelector('.diff .diff-words');
  assert.equal(words.querySelector('del').textContent, 'refused.');
  assert.equal(words.querySelector('ins').textContent, 'refused, and the link says so.');
  assert.match(words.textContent, /^An edit made through the link is /);
  assert.equal(view.querySelector('.diff-same'), null);
});

test('a changed node whose words did not change says that only its other fields changed', () => {
  const statement = 'While a list is shared, the shelf shall refuse edits made through the link.';
  const view = diffView([{ key: 'r', kind: 'requirement', label: 'RQ-7', name: statement, change: 'changed', words: { before: statement, after: statement } }]);
  assert.equal(view.querySelector('.diff .diff-words').textContent, statement);
  assert.equal(view.querySelector('.diff ins, .diff del'), null);
  assert.match(view.querySelector('.diff-same').textContent, /Only its other fields changed/);
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

test('the line joining a criterion to its Requirement runs from the Requirement’s right to the criterion’s left, inside their panel', async () => {
  const layout = trunks(await laid({ nodes: [product, group, feature, { ...requirement, change: 'added' }, criterion] }));
  const [held, beside] = [at(layout, 'r'), at(layout, 'c1')];
  const line = layout.edges.find((edge) => edge.pattern === 'tree');
  const [start, end] = [line.points[0], line.points.at(-1)];
  assert.ok(Math.abs(start.x - (held.x + held.width)) < 1 && start.y >= held.y && start.y <= held.y + held.height, 'from the Requirement');
  assert.ok(Math.abs(end.x - beside.x) < 1 && end.y >= beside.y && end.y <= beside.y + beside.height, 'to the criterion');
});

test('the lines from a Requirement to its criteria share one trunk, leaving and reaching each box at its head line', async () => {
  const more = [1, 2, 3].map((n) => ({ ...criterion, key: `c${n + 1}`, label: `AC-${30 + n}`, name: `Criterion number ${n} of the Requirement.` }));
  const layout = trunks(await laid({ nodes: [product, group, feature, { ...requirement, change: 'added' }, criterion, ...more] }));
  const held = at(layout, 'r');
  const lines = layout.edges.filter((edge) => edge.pattern === 'tree');
  assert.equal(lines.length, 4);
  const trunk = held.x + held.width + TRUNK;
  const head = (/** @type {any} */ node) => node.y + CARD.pad + CARD.head / 2;
  for (const line of lines) {
    const to = layout.nodes.find((node) => node.id === line.to);
    assert.deepEqual(line.points[0], { x: held.x + held.width, y: head(held) }, 'out from the Requirement’s head line');
    assert.deepEqual(line.points.at(-1), { x: to.x, y: head(to) }, 'into the criterion’s head line');
    assert.ok(line.points.length === 2 || line.points[1].x === trunk, 'along the one trunk');
  }
});
