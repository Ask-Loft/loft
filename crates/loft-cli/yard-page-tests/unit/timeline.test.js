// The timeline and the blocks drawn in it.

import { forget, root } from './dom.js';

import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';

import * as prefs from '../../yard-page/lib/prefs.js';
import { blocksByKey, changedSince, decisionsByQuestion, grouped, ordered, stretches } from '../../yard-page/model/log.js';
import { chart } from '../../yard-page/model/gantt.js';
import { card } from '../../yard-page/views/blocks.js';
import { logView, unreadable } from '../../yard-page/views/log.js';
import { timeline } from '../../yard-page/views/timeline.js';
import { EXPLORE, PERSON, act, answer, block, log, open, question, stage } from './fixtures.js';

beforeEach(forget);

/** A drawing context for `shown`. */
function context(shown, more = {}) {
  return {
    blocks: blocksByKey(shown),
    decisions: decisionsByQuestion(shown),
    marked: new Set(),
    waiting: new Set(),
    order: 'oldest',
    modes: [],
    now: Date.parse('2026-01-01T12:00:00Z'),
    ...more,
  };
}

/** A Yard Log's page, drawn into the document. */
function page(shown, more = {}) {
  const at = root();
  at.append(
    logView(shown, {
      tree: '/work/shelf',
      branch: 'feat/shelf',
      last: null,
      now: Date.parse('2026-01-01T12:00:00Z'),
      width: 960,
      refresh: () => {},
      notify: () => {},
      onView: () => {},
      onPick: () => {},
      ...more,
    }),
  );
  return at;
}

test('each active stage is open and every other stage is folded', () => {
  const shown = log([stage('done', 'Done one', 'done', []), stage('now', 'Now', 'active', []), stage('later', 'Later', 'planned', [])]);
  const drawn = timeline(shown, context(shown));
  const open = [...drawn.querySelectorAll('details.stage')].map((details) => [details.id, details.open]);
  assert.deepEqual(open, [
    ['block-done', false],
    ['block-now', true],
    ['block-later', false],
  ]);
});

test('each stage shows its title, its status as a word, its goal and its owner', () => {
  const owned = stage('scan', 'Scan', 'active', [], { owner: EXPLORE });
  const shown = log([owned]);
  const drawn = timeline(shown, context(shown));
  assert.equal(drawn.querySelector('.stage-title').textContent, 'Scan');
  assert.equal(drawn.querySelector('.status').textContent, 'Active');
  assert.match(drawn.querySelector('.goal').textContent, /The goal of Scan/);
  assert.equal(drawn.querySelector('.owner').textContent, 'Explore subagent');
});

test('a line marks where the stages done and dropped end and the planned ones begin', () => {
  const stages = [stage('a', 'A', 'done', []), stage('b', 'B', 'dropped', []), stage('c', 'C', 'planned', []), stage('d', 'D', 'planned', [])];
  assert.equal(ordered(stages, 'oldest').line, 2);
  assert.equal(ordered(stages, 'newest').line, 2);
  const shown = log(stages);
  const drawn = timeline(shown, context(shown));
  const items = [...drawn.children].map((item) => (item.classList.contains('rail-line') ? 'line' : item.querySelector('.stage').id));
  assert.deepEqual(items, ['block-a', 'block-b', 'line', 'block-c', 'block-d']);
});

test('the stages are newest first unless the person chose otherwise, and the choice is kept in the browser', () => {
  assert.equal(prefs.order(), 'newest');
  const stages = [stage('first', 'First', 'done', []), stage('second', 'Second', 'active', [])];
  assert.deepEqual(ordered(stages, prefs.order()).list.map((each) => each.id), ['second', 'first']);
  prefs.set('order', 'oldest');
  assert.equal(localStorage.getItem('yard.order'), 'oldest');
  assert.deepEqual(ordered(stages, prefs.order()).list.map((each) => each.id), ['first', 'second']);
});

test('a note shows its title, its markdown and its tone as a colour and a word', () => {
  const note = block('note', 'n1', { title: 'Mind it', markdown: 'x', html: '<p><strong>x</strong></p>', tone: 'warning', folded: false });
  const shown = log([stage('s', 'S', 'active', [note])]);
  const drawn = card(note, context(shown));
  assert.equal(drawn.querySelector('h4').textContent, 'Mind it');
  assert.equal(drawn.querySelector('.prose strong').textContent, 'x');
  assert.equal(drawn.querySelector('.tone-word').textContent, 'warning');
  assert.ok(drawn.classList.contains('tone--warning'));
});

test('a table in a block’s Markdown sits in a wrapper that scrolls sideways, and spans its block', () => {
  const table = '<table><thead><tr><th>Part</th><th>What changed</th></tr></thead><tbody><tr><td>Feature</td><td>Folded</td></tr></tbody></table>';
  const note = block('note', 'n1', { title: 'Done', markdown: '| Part | What changed |', html: `<p>Before</p>${table}`, folded: false });
  const drawn = card(note, context(log([stage('s', 'S', 'active', [note])])));
  const wrap = drawn.querySelector('.prose > .table-wrap');
  assert.ok(wrap);
  assert.equal(wrap.children.length, 1);
  assert.equal(wrap.firstElementChild.tagName, 'TABLE');
  assert.equal(wrap.querySelector('td').textContent, 'Feature');
  assert.equal(drawn.querySelector('.prose > p').textContent, 'Before');
});

test('a note written folded starts folded', () => {
  const note = block('note', 'n1', { title: 'Later', markdown: 'x', html: '<p>x</p>', folded: true });
  const drawn = card(note, context(log([stage('s', 'S', 'active', [note])])));
  const details = drawn.querySelector('details');
  assert.ok(details);
  assert.equal(details.open, false);
});

test('html that a block holds is shown as text and makes no element', () => {
  const note = block('note', 'n1', { markdown: '<script>x</script>', html: '<p>&lt;script&gt;x&lt;/script&gt;</p>', folded: false });
  const drawn = card(note, context(log([stage('s', 'S', 'active', [note])])));
  assert.ok(!drawn.querySelector('script'));
  assert.equal(drawn.querySelector('.prose').textContent, '<script>x</script>');
});

test('a question shows its prompt, its context and its options, with each recommended option marked', () => {
  const asked = question('shelf', ['Top', 'Bottom'], { contextHtml: '<p>Some context</p>' });
  const drawn = card(asked, context(log([stage('s', 'S', 'active', [asked])])));
  assert.equal(drawn.querySelector('.prompt').textContent, 'Which shelf?');
  assert.equal(drawn.querySelector('.context').textContent, 'Some context');
  const options = [...drawn.querySelectorAll('.option')].map((option) => [option.querySelector('.option-label').textContent, Boolean(option.querySelector('.badge--recommended'))]);
  assert.deepEqual(options, [
    ['Top', true],
    ['Bottom', false],
  ]);
});

test('an answered question folds to its answer, with who gave it, when and on which channel', () => {
  const asked = question('shelf', ['Top', 'Bottom'], {});
  asked.answer = answer(['Bottom'], 'terminal');
  const drawn = card(asked, context(log([stage('s', 'S', 'active', [asked])])));
  assert.equal(drawn.querySelector('.answer-text').textContent, 'Bottom');
  assert.match(drawn.querySelector('.answer .byline').textContent, /Ada Reader .* in the terminal/);
  assert.equal(drawn.querySelector('details.fold').open, false);
  assert.ok(drawn.querySelector('.option.is-chosen .option-label').textContent === 'Bottom');
});

test('an approval or a plan shows its markdown, and once decided its outcome with who, when and where', () => {
  const plan = block('plan', 'p1', { title: 'The plan', markdown: 'm', html: '<p>Do it</p>', planMode: true });
  plan.outcome = { key: 'k', verdict: 'rejected', reason: 'too wide', channel: 'page', act: act('2026-01-01T10:09:00Z', 4, PERSON) };
  const drawn = card(plan, context(log([stage('s', 'S', 'active', [plan])])));
  assert.equal(drawn.querySelector('.prose').textContent, 'Do it');
  assert.equal(drawn.querySelector('.verdict-word').textContent, 'Rejected');
  assert.match(drawn.querySelector('.outcome').textContent, /Ada Reader .* on the page: too wide/);
});

test('a block rejected in the terminal with no reason shows as rejected, with no reason given', () => {
  const plan = block('plan', 'p1', { title: 'The plan', markdown: 'm', html: '<p>Do it</p>', planMode: true });
  plan.outcome = { key: 'k', verdict: 'rejected', channel: 'terminal', act: act('2026-01-01T10:09:00Z', 4, PERSON) };
  const drawn = card(plan, context(log([stage('s', 'S', 'active', [plan])])));
  assert.equal(drawn.querySelector('.verdict-word').textContent, 'Rejected');
  assert.match(drawn.querySelector('.outcome').textContent, /in the terminal, with no reason given$/);
});

test('a task shows its title and its state', () => {
  const task = block('task', 't1', { title: 'Model it', state: 'active' });
  const drawn = card(task, context(log([stage('s', 'S', 'active', [task])])));
  assert.equal(drawn.querySelector('h4').textContent, 'Model it');
  assert.equal(drawn.querySelector('.state').textContent, 'Active');
});

test('a fix shows a mark that names it a fix beside its state, and a regular task shows none', () => {
  const fix = block('task', 't1', { title: 'Mend it', state: 'open', type: 'fix' });
  const regular = block('task', 't2', { title: 'Model it', state: 'open', type: 'regular' });
  const shown = context(log([stage('s', 'S', 'active', [fix, regular])]));
  const mark = card(fix, shown).querySelector('.block-head .state + .task-type');
  assert.equal(mark?.textContent, 'Fix');
  assert.ok(mark.classList.contains('task-type--fix'));
  assert.equal(card(regular, shown).querySelector('.task-type'), null);
});

test('every field that a block or a stage holds in Markdown is drawn from the HTML loft rendered from it', () => {
  const asked = question('shelf', ['Top', 'Bottom']);
  asked.version.body.options[0].detailHtml = '<p>The <strong>top</strong> one</p>';
  asked.answer = answer(['Top'], 'page');
  const settles = block('decision', 'd1', { title: 'Top', consequence: 'c', consequenceHtml: '<p>Ships <code>b84b656</code></p>', question: asked.key }, { more: { number: 1 } });
  const alone = block('decision', 'd2', { title: 'Alone', consequence: 'c', consequenceHtml: '<p>Keeps <em>it</em></p>' }, { more: { number: 2 } });
  const task = block('task', 't1', { title: 'Fix', state: 'done', detail: 'd', detailHtml: '<p>Also <code>5ddbbfe</code></p>' });
  const view = block('loft', 'v1', { view: 'findings', note: 'n', noteHtml: '<p>Shows <em>what changed</em></p>', reasons: [], recorded: { at: '2026-01-01T10:00:00Z' }, report: { findings: [] } });
  const shaped = stage('s', 'S', 'active', [asked, settles, alone, task, view]);
  shaped.version.body.goalHtml = '<p>The <em>store</em> reads</p>';
  const shown = log([shaped]);
  const drawn = timeline(shown, context(shown));
  const article = (kind) => drawn.querySelector(`article.block[data-kind="${kind}"]`);
  assert.equal(drawn.querySelector('.goal em').textContent, 'store');
  assert.equal(article('question').querySelector('.option-detail strong').textContent, 'top');
  assert.equal(article('question').querySelector('.decision-head .consequence code').textContent, 'b84b656');
  assert.equal(article('decision').querySelector('.consequence em').textContent, 'it');
  assert.equal(article('task').querySelector('.detail code').textContent, '5ddbbfe');
  assert.equal(article('loft').querySelector('.note em').textContent, 'what changed');
});

test('a field whose HTML the page server did not give is drawn as its text', () => {
  const task = block('task', 't1', { title: 'Fix', state: 'done', detail: 'Also `5ddbbfe`' });
  const drawn = card(task, context(log([stage('s', 'S', 'active', [task])])));
  assert.equal(drawn.querySelector('.detail').textContent, 'Also `5ddbbfe`');
  assert.ok(!drawn.querySelector('.detail code'));
});

test('each block offers its current author and version with its creation time labelled', () => {
  const note = block('note', 'n1', { markdown: 'x', html: '<p>x</p>', folded: false }, { by: EXPLORE });
  const drawn = card(note, context(log([stage('s', 'S', 'active', [note])])));
  const meta = drawn.querySelector('.block-meta');
  assert.equal(meta.querySelector('.who').textContent, 'Explore');
  assert.match(meta.textContent, /version 1/);
  assert.equal(meta.querySelector('time').getAttribute('datetime'), '2026-01-01T10:00:00Z');
  assert.match(meta.textContent, /Created/);
  assert.doesNotMatch(meta.textContent, /Updated/);
});

test('a block with more than one version offers each earlier version, with who wrote it and when', () => {
  const note = block('note', 'n1', { markdown: 'b', html: '<p>b</p>', folded: false });
  const first = { key: 'v0', number: 1, act: act('2026-01-01T09:00:00Z', 1, EXPLORE), body: { markdown: 'a', html: '<p>a</p>' } };
  note.version = { ...note.version, number: 2 };
  note.versions = [first, note.version];
  const drawn = card(note, context(log([stage('s', 'S', 'active', [note])])));
  const earlier = drawn.querySelector('details.versions');
  assert.equal(earlier.querySelector('summary').textContent, '1 earlier version');
  assert.match(earlier.querySelector('.version-head').textContent, /Version 1, by Explore/);
});

test('a decision is drawn at the head of the question it settles and given no card of its own', () => {
  const asked = question('shelf', ['Top', 'Bottom']);
  asked.answer = answer(['Top'], 'page');
  const decided = block('decision', 'd1', { title: 'Top shelf', consequence: 'Readers reach it', question: asked.key }, { more: { number: 1 } });
  const shown = log([stage('s', 'S', 'active', [asked, decided])]);
  const drawn = timeline(shown, context(shown));
  const cards = [...drawn.querySelectorAll('article.block')].map((each) => each.dataset.kind);
  assert.deepEqual(cards, ['question']);
  const head = drawn.querySelector('.decision-head');
  assert.match(head.textContent, /Decision 1.*Top shelf.*Readers reach it/);
  assert.equal(drawn.querySelector('.prompt').textContent, 'Which shelf?');
  assert.equal(drawn.querySelector('.answer-text').textContent, 'Top');
});

test('a question that more than one decision names is headed by each of them, in the order of their numbers', () => {
  const asked = question('shelf', ['Top', 'Bottom']);
  const second = block('decision', 'd2', { title: 'Second', consequence: 'b', question: asked.key }, { more: { number: 2 } });
  const first = block('decision', 'd1', { title: 'First', consequence: 'a', question: asked.key }, { more: { number: 1 } });
  const shown = log([stage('s', 'S', 'active', [asked, second, first])]);
  assert.deepEqual(decisionsByQuestion(shown).get(asked.key).map((each) => each.id), ['d1', 'd2']);
  const drawn = timeline(shown, context(shown));
  assert.deepEqual([...drawn.querySelectorAll('.decision-line h4')].map((each) => each.textContent), ['First', 'Second']);
});

test('the blocks written during one stretch of plan mode are drawn together under a band that names it', () => {
  const inside = block('note', 'in', { markdown: 'x', html: '', folded: false }, { at: '2026-01-01T10:01:00Z' });
  const outside = block('note', 'out', { markdown: 'y', html: '', folded: false }, { at: '2026-01-01T10:09:00Z' });
  const task = block('task', 'listed', { title: 'T', state: 'open' }, { at: '2026-01-01T10:10:00Z', more: { times: { listed: '2026-01-01T10:02:00Z' } } });
  const modes = [{ mode: 'plan', started: '2026-01-01T10:00:30Z', ended: '2026-01-01T10:03:00Z' }];
  const items = grouped([inside, task, outside], modes, Date.now());
  assert.equal(items.length, 2);
  assert.deepEqual(items[0].blocks.map((each) => each.id), ['in', 'listed']);
  const shown = log([stage('s', 'S', 'active', [inside, task, outside])]);
  const drawn = timeline(shown, context(shown, { modes }));
  const band = drawn.querySelector('.mode-band--plan');
  assert.match(band.querySelector('.mode-head').textContent, /Plan mode from .* for 3 min/);
  assert.equal(band.querySelectorAll('article.block').length, 2);
});

test('a band of spec mode is told apart from a band of plan mode by its class, its hatching and its label', () => {
  const inside = block('note', 'in', { markdown: 'x', html: '', folded: false }, { at: '2026-01-01T10:01:00Z' });
  const modes = [{ mode: 'spec', started: '2026-01-01T10:00:30Z' }];
  const shown = log([stage('s', 'S', 'active', [inside])]);
  const drawn = timeline(shown, context(shown, { modes }));
  const band = drawn.querySelector('.mode-band--spec');
  assert.ok(band);
  assert.match(band.querySelector('.mode-head').textContent, /Spec mode from .* and goes on/);
});

test('opening a Yard Log for the first time marks nothing', () => {
  const shown = log([stage('s', 'S', 'active', [question('shelf', ['A', 'B'])])]);
  assert.equal(changedSince(shown, null).size, 0);
});

test('each block written, answered or decided since the last visit is marked, and the page counts them', () => {
  const old = block('note', 'old', { markdown: 'x', html: '', folded: false }, { revision: 1 });
  const fresh = block('note', 'fresh', { markdown: 'y', html: '', folded: false }, { revision: 5 });
  const asked = question('shelf', ['A', 'B']);
  asked.answer = answer(['A'], 'terminal', '2026-01-01T10:05:00Z', 6);
  asked.responses = [asked.answer];
  const shown = log([stage('s', 'S', 'active', [old, fresh, asked])], { revision: 6 });
  assert.deepEqual([...changedSince(shown, 3)].sort(), [asked.key, fresh.key].sort());
  const drawn = page(shown, { last: 3 });
  assert.equal(drawn.querySelector('.changed-count').textContent, '2 blocks changed since your last visit');
  assert.equal(drawn.querySelectorAll('article.is-new').length, 2);
});

test('a block that a merge brought in since the last visit is marked', () => {
  const merged = block('note', 'merged', { markdown: 'x', html: '', folded: false }, { revision: 1, more: { reached: 7 } });
  const shown = log([stage('s', 'S', 'active', [merged])], { revision: 7 });
  assert.ok(changedSince(shown, 6).has(merged.key));
});

test('a question whose decision was written after the last visit is marked as changed', () => {
  const asked = question('shelf', ['A', 'B']);
  const decided = block('decision', 'd1', { title: 'A', consequence: 'c', question: asked.key }, { revision: 8, more: { number: 1 } });
  const shown = log([stage('s', 'S', 'active', [asked, decided])], { revision: 8 });
  assert.ok(changedSince(shown, 4).has(asked.key));
});

test('closed Yard Log shows when it was closed and offers nothing to answer or decide', () => {
  const asked = question('shelf', ['A', 'B']);
  const closed = log([stage('s', 'S', 'done', [asked])], {
    submissions: [open([asked], '2026-01-01T10:04:00Z')],
    yardLog: { key: 'k', title: 'Shelf', branch: 'feat/shelf', format: 'yardlog/1', closedBy: 'agent', closed: act('2026-01-01T11:00:00Z', 9) },
  });
  const drawn = page(closed);
  assert.match(drawn.querySelector('.banner--closed').textContent, /Closed .*Nothing here can be answered or decided/);
  assert.ok(!drawn.querySelector('.submission'));
  assert.ok(!drawn.querySelector('input'));
  const merged = page({ ...closed, yardLog: { ...closed.yardLog, closedBy: 'merge', closed: undefined } });
  assert.match(merged.querySelector('.banner--closed').textContent, /Closed by its merge into the base branch/);
});

test('Yard Log that does not read shows its errors, with their files and lines, in place of its stages', () => {
  const drawn = unreadable({ errors: [{ kind: 'unreadable', message: 'A stage file is not YAML.', sites: [{ path: '.yard/feat/shelf/s.yaml', line: 4 }] }] });
  assert.match(drawn.querySelector('h2').textContent, /does not read/);
  assert.match(drawn.querySelector('li').textContent, /A stage file is not YAML\..*\.yard\/feat\/shelf\/s\.yaml:4/);
  assert.ok(!drawn.querySelector('.timeline'));
});

test('Yard Log of a newer format names its format, says a newer loft is needed, and offers nothing to answer', () => {
  const asked = question('shelf', ['A', 'B']);
  const newer = log([stage('s', 'S', 'active', [asked])], {
    leftOut: 0,
    submissions: [open([asked], '2026-01-01T10:04:00Z')],
    yardLog: { key: 'k', title: 'Shelf', branch: 'feat/shelf', format: 'yardlog/2', closedBy: 'open' },
  });
  const drawn = page(newer);
  assert.match(drawn.querySelector('.banner--newer').textContent, /format yardlog\/2.*a newer loft is needed to show it/);
  assert.ok(!drawn.querySelector('.submission'));
});

test('an inherited Yard Log shows its open questions and offers no answer or decision', () => {
  const asked = question('shelf', ['A', 'B']);
  const inherited = log([stage('s', 'S', 'active', [asked])], { inherited: true, submissions: [open([asked], '2026-01-01T10:04:00Z')] });
  const drawn = page(inherited);
  assert.ok(drawn.querySelector('.banner--inherited'));
  assert.ok(drawn.querySelector(`article[data-key="${asked.key}"]`));
  assert.ok(!drawn.querySelector('.submission'));
  assert.ok(!drawn.querySelector('button.link'));
});

test('every host plan has the same Plan mode band and approval explanation without native activity', () => {
  const explanations = [];
  for (const host of ['claude-code', 'codex']) {
    for (const planMode of [true, false, undefined]) {
      const plan = block('plan', 'p1', { title: 'Build', markdown: 'x', html: '<p>x</p>', ...(planMode === undefined ? {} : { planMode }) }, { by: { host, session: 's1' } });
      const shown = log([stage('s', 'S', 'active', [plan])]);
      const drawn = timeline(shown, context(shown));
      const band = drawn.querySelector('.mode-band--plan');
      assert.ok(band, `${host} planMode=${planMode}`);
      assert.match(band.querySelector('.mode-head').textContent, /Plan mode from .* and goes on/);
      assert.equal(band.querySelectorAll('article.block').length, 1);
      const text = band.querySelector('.plan-mode').textContent;
      assert.match(text, /Approving authorizes the planned work, subject to the agent's mode and permissions/);
      assert.doesNotMatch(text, /auto mode/);
      explanations.push(text);
    }
  }
  assert.equal(new Set(explanations).size, 1);
});

test('a plan fallback band uses its current presentation and decision times and contains that plan alone', () => {
  const before = block('note', 'before', { html: '<p>Before</p>' });
  const plan = block('plan', 'p1', { title: 'Build', html: '<p>x</p>' }, { at: '2026-01-01T10:01:00Z' });
  plan.versions.unshift({ ...plan.version, act: act('2026-01-01T09:00:00Z', 1) });
  plan.outcome = { verdict: 'approved', channel: 'page', act: act('2026-01-01T10:04:00Z', 3, PERSON) };
  const after = block('note', 'after', { html: '<p>After</p>' }, { at: '2026-01-01T10:02:00Z' });
  const shown = log([stage('s', 'S', 'active', [before, plan, after])]);
  const original = structuredClone(shown);
  const beforeChart = chart(shown, Date.parse('2026-01-01T12:00:00Z'));
  const items = grouped([before, plan, after], [], Date.now());
  assert.equal(items.length, 3);
  assert.deepEqual(items[1].stretch, { mode: 'plan', started: plan.version.act.at, ended: plan.outcome.act.at });
  assert.deepEqual(items[1].blocks, [plan]);
  const drawn = timeline(shown, context(shown));
  assert.match(drawn.querySelector('.mode-head').textContent, /Plan mode from .* for 3 min/);
  assert.equal(drawn.querySelectorAll('.mode-band article.block').length, 1);
  assert.equal(drawn.querySelectorAll('article.block').length, 3);
  assert.deepEqual(shown, original);
  assert.deepEqual(stretches(shown), []);
  assert.deepEqual(chart(shown, Date.parse('2026-01-01T12:00:00Z')), beforeChart);
  assert.deepEqual(beforeChart.bands, []);
});

test('a plan within native plan activity reuses its band once and otherwise receives its own Plan mode band', () => {
  const plan = block('plan', 'p1', { title: 'Build', html: '' }, { at: '2026-01-01T10:01:00Z' });
  const note = block('note', 'n1', { html: '' }, { at: '2026-01-01T10:02:00Z' });
  const span = { mode: 'plan', started: '2026-01-01T10:00:00Z', ended: '2026-01-01T10:03:00Z' };
  const shown = log([stage('s', 'S', 'active', [plan, note])]);
  const items = grouped([plan, note], [span], Date.now());
  assert.equal(items.length, 1);
  assert.equal(items[0].stretch, span);
  const drawn = timeline(shown, context(shown, { modes: [span] }));
  assert.equal(drawn.querySelectorAll('.mode-band--plan').length, 1);
  assert.equal(drawn.querySelectorAll('article[data-kind="plan"]').length, 1);
  const spec = { ...span, mode: 'spec' };
  const fallback = timeline(shown, context(shown, { modes: [spec] }));
  assert.equal(fallback.querySelectorAll('.mode-band--plan article').length, 1);
  assert.equal(fallback.querySelector('.mode-band--plan article').dataset.kind, 'plan');
  assert.equal(fallback.querySelector('.mode-band--spec article').dataset.kind, 'note');
});
