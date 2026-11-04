// The `loft` views: spec change, check findings, trace coverage, decision and publish state.

import { forget } from './dom.js';

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { beforeEach, test } from 'node:test';
import { gunzipSync } from 'node:zlib';

import { add, h } from '../../yard-page/lib/dom.js';
import { decisionView } from '../../yard-page/views/loft/decision.js';
import { byRule, findingsView } from '../../yard-page/views/loft/findings.js';
import { loftView, named } from '../../yard-page/views/loft/index.js';
import { publishView } from '../../yard-page/views/loft/publish.js';
import { CARD, CHIP, MOST_BOXES, TRUNK, boxes, cut, evenPanels, extras, headOf, specGraph, trunks, unconstrained, wrap } from '../../yard-page/model/spec-graph.js';
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

test('a box gives its node’s label in monospace, or its kind’s label prefix where the node has none yet', () => {
  assert.equal(headOf({ ...criterion, label: null }).label, 'AC');
  assert.equal(headOf({ ...requirement, label: null }).label, 'RQ');
  assert.equal(headOf({ key: 'd', kind: 'decision', label: null, name: 'x' }).label, 'ADR');
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
  const [error] = view.querySelectorAll('.finding');
  assert.match(error.textContent, /parse error The file is not YAML\./);
  assert.equal(error.querySelector('.site code').textContent, 'spec/b.yaml:2');
  const first = view.querySelector('.finding-group[data-rule="LQ-005"]');
  assert.match(first.querySelector('summary').textContent, /LQ-005 info The text may hold more than one statement\./);
  const place = first.querySelector('.finding-line .site');
  assert.equal(place.textContent, 'a.yaml:7', 'its file’s name and line');
  assert.equal(place.getAttribute('title'), 'spec/a.yaml:7', 'its whole path on hover');
  assert.equal(place.querySelector('.site-line').textContent, ':7');
  assert.match(css, /\.site-file \{ min-width: 0; overflow: hidden; text-overflow: ellipsis; \}/, 'a long file name is cut');
  assert.match(css, /\.site-line \{ flex: none; \}/, 'never its line');
  assert.match(view.querySelector('.base').textContent, /against develop/);
});

test('a finding names its node by label and name, is marked introduced or there before, and shows the reason it was left', () => {
  const report = { check: { errors: [], findings: [finding({ change: 'preexisting' })], resolved: [finding({ rule: 'LQ-009' })] }, nodes: { n1: { label: 'AC-4', name: 'A link expires' } } };
  const view = drawn(findingsView(report, { reasons: [{ rule: 'LQ-005', key: 'n1', reason: 'one rule, said once' }] }));
  assert.equal(view.querySelector('.finding-line .finding-label').textContent, 'AC-4');
  assert.equal(view.querySelector('.finding-line .node').textContent, 'A link expires');
  assert.equal(view.querySelector('.finding-group .finding-heading').textContent.trim(), 'There before (1)');
  assert.match(view.querySelector('.finding-line .reason').textContent, /one rule, said once/);
  assert.match(view.querySelector('.resolved summary').textContent, /1 resolved by the branch/);
});

test('findings are grouped by rule from errors down to hints, each rule giving its message once and counting its new and older findings', () => {
  const findings = [
    finding({ rule: 'LQ-008', severity: 'info', message: 'Near-duplicates.', change: 'preexisting', subjects: [{ key: 'a' }] }),
    finding({ rule: 'LQ-009', severity: 'hint', message: 'A tag term.', change: 'preexisting', subjects: [{ key: 'b' }] }),
    finding({ rule: 'LQ-008', severity: 'info', message: 'Near-duplicates.', change: 'introduced', subjects: [{ key: 'c' }] }),
    finding({ rule: 'LQ-002', severity: 'warning', message: 'A vague word.', change: 'preexisting', subjects: [{ key: 'd' }] }),
    finding({ rule: 'LQ-008', severity: 'info', message: 'Near-duplicates.', change: 'preexisting', subjects: [{ key: 'e' }] }),
  ];
  assert.deepEqual(byRule(findings).map((group) => [group.rule, group.introduced.length, group.before.length]), [['LQ-002', 0, 1], ['LQ-008', 1, 2], ['LQ-009', 0, 1]]);
  const nodes = Object.fromEntries(['a', 'b', 'c', 'd', 'e'].map((key) => [key, { label: `AC-${key}`, name: `Node ${key}` }]));
  const view = drawn(findingsView({ check: { errors: [], findings, resolved: [] }, nodes }, { reasons: [] }));
  const marks = (/** @type {Element} */ within) => [...within.querySelectorAll('.count-mark')].map((mark) => [mark.textContent, mark.getAttribute('aria-label'), mark.getAttribute('title'), Boolean(mark.querySelector('svg path'))]);
  assert.deepEqual(marks(view.querySelector('.finding-split')), [['1', '1 new on this branch', '1 new on this branch', true], ['4', '4 there before', '4 there before', true]], 'each count a number beside its icon, named');
  const groups = [...view.querySelectorAll('.finding-group')];
  assert.deepEqual(groups.map((group) => group.dataset.rule), ['LQ-002', 'LQ-008', 'LQ-009'], 'most severe first');
  const dupes = groups[1];
  assert.equal(dupes.querySelectorAll('.finding-message').length, 1, 'its message once');
  assert.deepEqual(marks(dupes.querySelector('summary')).map(([, words]) => words), ['1 new on this branch', '2 there before']);
  assert.deepEqual(groups.map((group) => group.open), [false, true, false], 'open only where the branch introduced one');
  assert.deepEqual([...dupes.querySelectorAll('.finding-heading')].map((heading) => heading.textContent.trim()), ['New on this branch (1)', 'There before (2)']);
  assert.equal(dupes.querySelector('details.finding-before')?.open, false, 'beneath new findings, those there before are folded beneath their count');
  assert.equal(groups[0].querySelector('details.finding-before'), null, 'in a group of older findings alone, they are listed');
  const css = readFileSync(new URL('../../yard-page/yard.css', import.meta.url), 'utf8');
  assert.match(css, /\.finding-what \{ min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; \}/, 'one line, cut where it runs out of room');
  assert.equal(dupes.querySelector('.finding-what')?.getAttribute('title'), 'Node c', 'its whole words on hover');
  assert.deepEqual([...dupes.querySelectorAll('.finding-line')].map((line) => [line.querySelector('.finding-label').textContent, line.querySelector('.node').textContent]), [['AC-c', 'Node c'], ['AC-a', 'Node a'], ['AC-e', 'Node e']], 'the new first, each on one line');
  assert.match(css, /\.finding-line \{ display: grid; grid-template-columns: 6\.5em minmax\(0, 1fr\) minmax\(0, 16em\);[^}]*border-top: 1px solid var\(--line\); \}/, 'columns aligned from line to line, a hairline between');
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
  assert.equal(view.querySelector('.decision-option, .decision-options'), null, 'the options are listed once, as the table’s rows');
  assert.equal(view.querySelector('.assessments tr.is-chosen th').textContent, 'One file (selected)');
  const cells = [...view.querySelectorAll('.assessments tbody tr:not(.option-why) td')];
  assert.deepEqual(cells.map((cell) => cell.querySelector('.rating-symbol').textContent), ['✓', '✕'], 'a symbol');
  assert.deepEqual(cells.map((cell) => cell.querySelector('.rating .sr-only')?.textContent.trim()), ['supports', 'conflicts'], 'its word named to a reader alone');
  assert.deepEqual([...view.querySelectorAll('.rating-key .rating')].map((mark) => mark.textContent), ['✓ supports', '– neutral', '✕ conflicts'], 'a line above the table names each symbol');
  assert.deepEqual(cells.map((cell) => cell.querySelector('.rating-why').textContent), ['one read', 'many reads']);
  assert.equal(view.querySelector('.assessments thead th:nth-child(2)').textContent, 'Speed');
  const tones = cells.map((cell) => cell.querySelector('.rating').className);
  assert.equal(new Set(tones).size, 2, `each rating has its own colour: ${tones}`);
  assert.match([...view.querySelectorAll('details li')].map((item) => item.textContent).join('\n'), /Speed Fast reads/);
  assert.equal(view.querySelector('.consequence[data-kind="risk"] .consequence-kind').textContent, 'risk');
  assert.match(view.querySelector('.consequence').textContent, /A big file/);
  const affects = view.querySelector('.subject-role-row[data-role="affected"]');
  assert.equal(affects.querySelector('.subject-role').textContent, 'Affects');
  assert.match(affects.querySelector('.subject').textContent, /^CMP-1\s+Shelf store/);
  assert.equal(view.querySelector('.subjects .chip, .consequences .chip'), null, 'no framed tag');
});

test('a decision’s selected option comes first in its table, marked, each row opens to its rationales, and a cell shows its rationale on hover or focus', () => {
  const report = {
    decision: {
      key: 'd', label: 'ADR-3', title: 'T', state: { kind: 'accepted' },
      drivers: [{ key: 'r1', label: 'Speed' }, { key: 'r2', label: 'Size' }],
      options: [{ key: 'o1', title: 'First' }, { key: 'o2', title: 'Chosen', selected: true, description: 'The one' }],
      assessments: [{ option: 'o2', driver: 'r1', rating: 'supports', rationale: 'quick' }, { option: 'o2', driver: 'r2', rating: 'neutral', rationale: 'middling' }],
      consequences: [{ kind: 'benefit', statement: 'B1' }, { kind: 'cost', statement: 'C1' }, { kind: 'benefit', statement: 'B2' }],
      subjects: [{ role: 'affected', node: { label: 'F-1', name: 'One' } }, { role: 'affected', node: { label: 'F-2', name: 'Two' } }, { role: 'outcome', node: { label: 'RQ-1', name: 'Out' } }],
    },
  };
  const view = drawn(decisionView(report));
  const rows = [...view.querySelectorAll('.assessments tbody tr:not(.option-why)')];
  assert.deepEqual(rows.map((row) => row.querySelector('.option-open').textContent), ['Chosen', 'First']);
  assert.ok(rows[0].classList.contains('is-chosen'));
  const toggle = rows[0].querySelector('.option-open');
  const why = view.querySelector(`#${toggle.getAttribute('aria-controls')}`);
  assert.equal(why.hidden, true);
  toggle.click();
  assert.equal(toggle.getAttribute('aria-expanded'), 'true');
  assert.equal(why.hidden, false);
  assert.match(why.textContent, /The one/);
  const ratings = [...why.querySelectorAll('.option-ratings > *')].map((each) => each.textContent.trim());
  assert.deepEqual(ratings, ['Speed', '✓ supports', 'quick', 'Size', '– neutral', 'middling'], 'driver, rating and rationale, side by side');
  assert.match(css, /\.assessments \.option-why > td \{[^}]*text-align: left;/, 'left-aligned, not centred as the ratings are');
  assert.match(css, /\.assessments tr:not\(\.option-why\) > td:has\(\.rating\) \{ text-align: center; \}/);
  assert.match(css, /\.option-why-body \{ position: sticky; left: 8px; max-width: min\(720px, var\(--room, 720px\)\);/, 'held within the width the table is seen in');
  assert.equal(rows[0].querySelector('td').getAttribute('tabindex'), '0', 'reached by the keyboard');
  assert.match(css, /\.assessments td:is\(:hover, :focus-visible\) \.rating-why \{ display: block; \}/);
  assert.match(css, /\.assessments tr\.is-chosen > th \{[^}]*box-shadow: inset 3px 0 0 var\(--signal\);/, 'a bar at its left');
  assert.deepEqual([...view.querySelectorAll('ul.consequences > li.consequence')].map((item) => item.textContent), ['benefitB1', 'costC1', 'benefitB2'], 'one list, each led by its kind');
  assert.match(css, /\.consequence-kind \{[^}]*color: var\(--muted\);/, 'its kind a small muted word');
  const roleRows = (/** @type {Element} */ row) => [row.querySelector('.subject-role').textContent, [...row.querySelectorAll('.subject')].map((item) => item.textContent.replace(/\s+/g, ' ').trim())];
  assert.deepEqual([...view.querySelectorAll('.subject-role-row')].map(roleRows), [['Affects', ['F-1 One', 'F-2 Two']], ['Stated as', ['RQ-1 Out']]], 'one subject to a line beside its role');
  assert.equal(view.querySelector('.subject')?.getAttribute('title'), 'F-1 One', 'its name whole on hover');
  assert.match(css, /\.subject-role-row \{ display: grid; grid-template-columns: 7\.5em minmax\(0, 1fr\);/, 'in the column of the consequences’ kinds');
  assert.match(css, /\.consequences > \.consequence \{ display: grid; grid-template-columns: 7\.5em minmax\(0, 1fr\);/);
  assert.match(css, /\.subject \{ min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; \}/, 'kept to one line');
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

test('in its block, each box is a chip of one line, its name cut to fit, and its whole name given to a reader and under the pointer', async () => {
  const layout = placed(await new ELK().layout(specGraph({ nodes: [product, group, feature, { ...requirement, change: 'added' }, criterion], decisions: [], edges: [] }, null, measure, true)));
  const chip = at(layout, 'r');
  assert.equal(chip.card.compact, true);
  assert.equal(chip.height, CHIP.height, 'one line');
  assert.ok(chip.card.shown.endsWith('…') && measure([chip.card.shown]).width <= CHIP.name, 'its name cut to fit');
  const drawn = flowchartSvg(layout, 'c1');
  const shown = drawn.querySelector('[data-key="r"]');
  assert.equal(shown?.querySelector('title')?.textContent, requirement.name, 'under the pointer');
  assert.match(shown?.getAttribute('aria-label') ?? '', new RegExp(requirement.name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
  assert.equal(shown?.querySelector('.spec-word')?.textContent, 'NEW');
  assert.equal(cut('short', 100, measure), 'short', 'a name that fits is not cut');
});

test('Components that nest are drawn, a chain of them running to the right, and a layout ELK refuses is drawn without its columns', async () => {
  const top = { key: 'k1', kind: 'component', label: 'CMP-7', name: 'Science', parent: null, change: null };
  const middle = { key: 'k2', kind: 'component', label: 'CMP-31', name: 'Sources', parent: 'k1', change: null };
  const added = { key: 'k3', kind: 'component', label: null, name: 'Researched stars', parent: 'k2', change: 'added' };
  for (const compact of [true, false]) {
    const layout = placed(await new ELK().layout(specGraph({ nodes: [product, group, feature, top, middle, added], decisions: [], edges: [{ from: 'f', to: 'k3', kind: 'realizes' }] }, null, measure, compact)));
    assert.ok(at(layout, 'k1').x < at(layout, 'k2').x && at(layout, 'k2').x < at(layout, 'k3').x, 'the chain runs to the right');
  }
  const pinned = { id: 'root', layoutOptions: {}, children: [{ id: 'a', layoutOptions: { 'elk.layered.layering.layerConstraint': 'LAST' } }], edges: [] };
  assert.equal(unconstrained(pinned).children[0].layoutOptions['elk.layered.layering.layerConstraint'], undefined);
});

test('the boxes of one column start at one left edge, however wide each is', async () => {
  const short = { ...requirement, key: 'r2', label: 'RQ-8', name: 'Short.', change: 'added' };
  for (const compact of [true, false]) {
    const layout = placed(await new ELK().layout(specGraph({ nodes: [product, group, feature, { ...requirement, change: 'added' }, criterion, short], decisions: [], edges: [] }, null, measure, compact)));
    assert.equal(at(layout, 'r').x, at(layout, 'r2').x);
  }
});

test('panels that stand one beneath another in a column are as wide as one another, and a panel spans the panel that holds it', async () => {
  const other = { key: 'p2', kind: 'product', label: 'P-2', name: 'A second Product with a much longer name than the first one has', parent: null, change: null };
  const second = { key: 'g2', kind: 'feature_group', label: 'FG-9', name: 'Its group', parent: 'p2', change: null };
  const lone = { key: 'f2', kind: 'feature', label: 'F-9', name: 'Short', parent: 'g2', change: 'added' };
  for (const compact of [true, false]) {
    const layout = evenPanels(placed(await new ELK().layout(specGraph({ nodes: [product, group, feature, other, second, lone], decisions: [], edges: [] }, null, measure, compact))));
    const [p1, p2] = [at(layout, 'p'), at(layout, 'p2')];
    assert.equal(p1.x, p2.x, 'one column');
    assert.equal(p1.width, p2.width, 'as wide as one another');
    for (const [inner, outer] of [['g', 'p'], ['f', 'g'], ['g2', 'p2'], ['f2', 'g2']]) {
      const [held, holder] = [at(layout, inner), at(layout, outer)];
      assert.equal(held.x + held.width, holder.x + holder.width - holder.panel.pad, `${inner} spans ${outer}`);
    }
  }
});

test('a decision’s parts stand on one rhythm, its head and outcome and its two folds as pairs, its consequences’ kinds quiet words', () => {
  assert.match(css, /\.loft-view--decision \{ gap: 16px; \}/);
  assert.match(css, /\.loft-view--decision > \.decision-view-head \+ \.outcome \{ margin-top: -8px;/);
  assert.match(css, /\.loft-view--decision > details\.fold \+ details\.fold \{ margin-top: -10px; \}/);
  assert.match(css, /\.consequence-kind \{ color: var\(--muted\); font-size: 13px; font-weight: 500; text-transform: capitalize; \}/, 'not mono capitals');
  assert.match(css, /^\.consequences > \.consequence \{ display: grid;/m, 'the kind column only within a Loft decision’s list of consequences');
  assert.doesNotMatch(css, /^\.consequence \{[^}]*grid/m, 'never on every consequence, which a Yard decision uses for its paragraph');
});

test('a decision’s outcome comes first beneath its head, and its context and drivers are folded at its end', () => {
  const view = drawn(
    decisionView({ decision: { title: 'T', label: 'ADR-5', context: 'Why we chose', state: { kind: 'accepted', outcome: 'We chose A' }, drivers: [{ key: 'r', label: 'Speed' }], options: [{ key: 'a', title: 'A', selected: true }], assessments: [] } }),
  );
  const order = [...view.children].map((child) => child.className.split(' ').find((name) => ['decision-view-head', 'outcome', 'decision-matrix', 'decision-context', 'fold'].includes(name)) ?? child.className);
  assert.equal(order[0], 'decision-view-head');
  assert.equal(order[1], 'outcome', 'the outcome first');
  assert.ok(order.indexOf('decision-matrix') > 1);
  const context = view.querySelector('details.decision-context');
  assert.equal(context?.open, false);
  assert.equal(context?.querySelector('summary')?.textContent, 'Context');
  assert.equal([...view.querySelectorAll('details')].at(-1)?.querySelector('summary')?.textContent, '1 driver');
  const css = readFileSync(new URL('../../yard-page/yard.css', import.meta.url), 'utf8');
  assert.match(css, /\.assessments th\[scope="row"\] \{ white-space: nowrap; \}/, 'an option’s title on one line');
});

test('a finding’s node without a label yet shows a muted mark that says it gets one at the publish, and a pair finding gives its second node beneath', () => {
  const pair = finding({ rule: 'LQ-008', message: 'Near-duplicates.', change: 'introduced', path: 'spec/one.yaml', line: 3, subjects: [{ key: 'a', path: 'spec/one.yaml', line: 3 }, { key: 'b', path: 'spec/sub/two.yaml', line: 9 }] });
  const nodes = { a: { label: null, name: 'The hooks reach the log' }, b: { label: 'RQ-9', name: 'The hooks reach the log only' } };
  const view = drawn(findingsView({ check: { errors: [], findings: [pair], resolved: [] }, nodes }, { reasons: [] }));
  const line = view.querySelector('.finding-line');
  const labels = [...line.querySelectorAll('.finding-label')];
  assert.equal(labels.length, 2, 'the second node in the same columns');
  const mark = labels[0].querySelector('.no-label');
  assert.equal(mark?.textContent, 'no label');
  assert.match(mark?.getAttribute('title') ?? '', /gets one when the specification is published/);
  assert.equal(mark?.getAttribute('aria-label'), mark?.getAttribute('title'));
  assert.equal(labels[1].textContent, 'RQ-9');
  const second = line.querySelector('.finding-what.finding-second');
  assert.match(second.textContent, /≈ and The hooks reach the log only/);
  assert.deepEqual([...line.querySelectorAll('.site')].map((site) => [site.textContent, site.getAttribute('title')]), [['one.yaml:3', 'spec/one.yaml:3'], ['two.yaml:9', 'spec/sub/two.yaml:9']]);
  assert.match(css, /\.no-label \{ color: var\(--muted\);/);
});
