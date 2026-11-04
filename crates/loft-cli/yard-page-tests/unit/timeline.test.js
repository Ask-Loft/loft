// The timeline and the blocks drawn in it.

import { forget, root } from './dom.js';

import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';

import * as prefs from '../../yard-page/lib/prefs.js';
import { blocksByKey, changedSince, decisionsByQuestion, grouped, ordered, stretches } from '../../yard-page/model/log.js';
import { chart } from '../../yard-page/model/gantt.js';
import { card } from '../../yard-page/views/blocks.js';
import { failedLog, logHead, logView, unreadable } from '../../yard-page/views/log.js';
import { KIND_ICONS, LIST, blockState, chosenStage, timeline } from '../../yard-page/views/timeline.js';
import { format, parse } from '../../yard-page/lib/route.js';
import { readFileSync } from 'node:fs';
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
  const drawing = {
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
  };
  const head = document.createElement('header');
  head.id = 'head';
  head.append(...logHead(shown, drawing));
  at.append(head, logView(shown, drawing));
  return at;
}

/** Whether each stage of the list is open to its blocks, by its row's id. */
function folds(drawn) {
  return [...drawn.querySelectorAll('.stages > li.station')].map((row) => [row.id, row.querySelector('.fold-button')?.getAttribute('aria-expanded') === 'true' && !row.querySelector('.block-rows')?.hidden]);
}

test('in the list, each active stage and the stage chosen are open to their blocks, and every other stage is folded', () => {
  const shown = log([stage('done', 'Done one', 'done', []), stage('now', 'Now', 'active', []), stage('later', 'Later', 'planned', [])]);
  assert.deepEqual(folds(timeline(shown, context(shown))), [
    ['row-done', false],
    ['row-now', true],
    ['row-later', false],
  ]);
  assert.deepEqual(folds(timeline(shown, context(shown, { chosen: { stage: 'later' } }))), [
    ['row-done', false],
    ['row-now', true],
    ['row-later', true],
  ]);
});

test('the person folds a stage of the list to its row and opens it to list its blocks by kind and title', () => {
  const note = block('note', 'n', { title: 'What it found', markdown: 'x', html: '' });
  const asked = question('shelf', ['A', 'B']);
  const shown = log([stage('now', 'Now', 'active', [note, asked]), stage('next', 'Next', 'planned', [])]);
  const drawn = timeline(shown, context(shown));
  const row = drawn.querySelector('#row-now');
  const rows = [...row.querySelectorAll('.block-rows .block-row')].map((each) => [each.querySelector('.kind-icon').getAttribute('aria-label'), each.querySelector('.block-row-title').textContent]);
  assert.deepEqual(rows, [
    ['Note', 'What it found'],
    ['Question', 'shelf'],
  ]);
  const fold = row.querySelector('.fold-button');
  fold.click();
  assert.equal(fold.getAttribute('aria-expanded'), 'false');
  assert.ok(row.querySelector('.block-rows').hidden);
  assert.ok(row.querySelector('.stage-choose'), 'folded, the stage keeps its row');
  fold.click();
  assert.ok(!row.querySelector('.block-rows').hidden);
});

test('the stage chosen is the one the address names, else the first active stage in the order shown, else the first stage', () => {
  const stages = [stage('a', 'A', 'done', [block('note', 'in-a', { markdown: 'x' })]), stage('b', 'B', 'active', []), stage('c', 'C', 'active', []), stage('d', 'D', 'planned', [])];
  assert.equal(chosenStage(stages, { stage: 'd' }).id, 'd');
  assert.equal(chosenStage(stages, { block: 'in-a' }).id, 'a', 'a block the address names chooses its stage');
  assert.equal(chosenStage(stages, {}).id, 'b');
  assert.equal(chosenStage([...stages].reverse(), { stage: 'gone' }).id, 'c');
  assert.equal(chosenStage([stages[0], stages[3]], {}).id, 'a');
});

test('choosing a stage shows it in the pane beside the list, with its goal, its status, its owner and each of its blocks', () => {
  const note = block('note', 'n', { title: 'Found', markdown: 'x', html: '<p>Found it.</p>' });
  const shown = log([stage('a', 'A', 'done', [note], { owner: EXPLORE }), stage('b', 'B', 'active', [])]);
  /** @type {any[]} */
  const chose = [];
  const drawn = timeline(shown, context(shown, { onChoose: (...args) => chose.push(args) }));
  assert.equal(drawn.querySelector('.stage-pane #stage-title').textContent, 'B');
  drawn.querySelector('#row-a .stage-choose').click();
  assert.deepEqual(chose, [['a']]);
  const again = timeline(shown, context(shown, { chosen: { stage: 'a' } }));
  const pane = again.querySelector('.stage-pane');
  assert.equal(pane.querySelector('#stage-title').textContent, 'A');
  assert.equal(pane.querySelector('.pane-kicker .status').textContent, 'Done');
  assert.equal(pane.querySelector('.pane-kicker .owner').textContent, 'Explore subagent');
  assert.match(pane.querySelector('.goal').textContent, /The goal of A/);
  assert.deepEqual([...pane.querySelectorAll('article.block')].map((each) => each.id), ['block-n']);
  assert.equal(again.querySelector('#row-a .stage-choose').getAttribute('aria-current'), 'true');
});

test('choosing a block in the list chooses its stage and the block, which the pane marks as chosen', () => {
  const note = block('note', 'n', { title: 'Found', markdown: 'x', html: '' });
  const shown = log([stage('a', 'A', 'active', [note]), stage('b', 'B', 'planned', [])]);
  /** @type {any[]} */
  const chose = [];
  timeline(shown, context(shown, { onChoose: (...args) => chose.push(args) })).querySelector('#row-a .block-row').click();
  assert.deepEqual(chose, [['a', 'n']]);
  const drawn = timeline(shown, context(shown, { chosen: { stage: 'a', block: 'n' } }));
  assert.ok(drawn.querySelector('#row-a .block-row').classList.contains('is-chosen'));
});

test('the address names the stage and the block chosen, so that a reload or a link opens them again', () => {
  const hash = format({ tree: '/work/shelf', branch: 'feat/shelf', stage: 'a', block: 'n' });
  assert.deepEqual(parse(hash), { tree: '/work/shelf', branch: 'feat/shelf', stage: 'a', block: 'n' });
  assert.deepEqual(parse(format({ tree: '/t', branch: 'b' })), { tree: '/t', branch: 'b', stage: undefined, block: undefined });
});

test('a stage’s row gives the count of its blocks that wait on the person and of those marked since the last visit', () => {
  const asked = question('shelf', ['A', 'B']);
  const fresh = block('note', 'fresh', { markdown: 'y' });
  const shown = log([stage('a', 'A', 'active', [asked, fresh]), stage('b', 'B', 'planned', [])]);
  const drawn = timeline(shown, context(shown, { waiting: new Set([asked.key]), marked: new Set([asked.key, fresh.key]) }));
  const facts = drawn.querySelector('#row-a .stage-facts');
  assert.equal(facts.querySelector('.waiting-count').textContent, '1 waiting');
  assert.equal(facts.querySelector('.changed-mark').textContent, '2 new');
  const quiet = timeline(shown, context(shown));
  assert.equal(quiet.querySelectorAll('#row-a .waiting-count, #row-a .changed-mark').length, 0);
});

test('a splitter between the list and the pane, reached by the keyboard, narrows and widens the list, and the browser keeps the width', () => {
  prefs.set('split', null);
  const shown = log([stage('a', 'A', 'active', [])]);
  const drawn = timeline(shown, context(shown));
  const handle = drawn.querySelector('.splitter');
  assert.equal(handle.getAttribute('role'), 'separator');
  assert.equal(handle.getAttribute('tabindex'), '0');
  assert.equal(drawn.style.getPropertyValue('--list'), `${LIST.width}px`);
  handle.dispatchEvent(Object.assign(new Event('keydown'), { key: 'ArrowRight' }));
  assert.equal(drawn.style.getPropertyValue('--list'), `${LIST.width + LIST.step}px`);
  handle.dispatchEvent(Object.assign(new Event('keydown'), { key: 'ArrowLeft' }));
  handle.dispatchEvent(Object.assign(new Event('keydown'), { key: 'ArrowLeft' }));
  assert.equal(prefs.get('split'), String(LIST.width - LIST.step));
  assert.equal(timeline(shown, context(shown)).style.getPropertyValue('--list'), `${LIST.width - LIST.step}px`);
  prefs.set('split', null);
});

test('the list and the pane each scroll on their own, and the page itself never scrolls', () => {
  const css = readFileSync(new URL('../../yard-page/yard.css', import.meta.url), 'utf8');
  assert.match(css, /\.stage-list, \.stage-pane \{[^}]*overflow-y: auto/);
  assert.match(css, /\.page \{[^}]*height: 100dvh;[^}]*overflow: hidden/);
  assert.match(css, /\.layout \{[^}]*height: 100dvh;[^}]*overflow: hidden/);
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
  const items = [...drawn.querySelector('.stages').children].map((item) => (item.classList.contains('rail-line') ? 'line' : item.id));
  assert.deepEqual(items, ['row-a', 'row-b', 'line', 'row-c', 'row-d']);
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

test('an address whose tree the page server does not serve names the tree and how to add it, and no unreadable files', () => {
  const tree = '/work/shelf/.claude/worktrees/calm-oak';
  const refused = { errors: [{ kind: 'tree', message: `the page server serves no tree ${tree}`, sites: [] }] };
  const drawn = failedLog(404, refused, tree);
  assert.match(drawn.querySelector('h2').textContent, /does not serve this tree/);
  assert.equal(drawn.querySelector('code.tree').textContent, tree);
  assert.match(drawn.textContent, /No session has registered it with the page server/);
  assert.equal(drawn.querySelector('code.command').textContent, `loft yard serve --tree ${tree}`);
  assert.doesNotMatch(drawn.textContent, /cannot be read|does not read/);
  const broken = failedLog(409, { errors: [{ kind: 'unreadable', message: 'A stage file is not YAML.', sites: [] }] }, tree);
  assert.match(broken.querySelector('h2').textContent, /does not read/);
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

/** A lone stage that holds a note, a task, a question and an approval. */
function lone() {
  const note = block('note', 'found', { title: 'What it found', markdown: 'x', html: '' });
  const task = block('task', 'build', { title: 'Build the shelf', state: 'active' });
  const asked = question('shelf', ['A', 'B']);
  const approval = block('approval', 'ship', { title: 'Ship it', markdown: 'y' }, { more: { outcome: { verdict: 'approved' } } });
  return { note, task, asked, approval, shown: log([stage('only', 'Only', 'active', [note, task, asked, approval])]) };
}

test('with one stage, the list holds a row for each of its blocks in their order, and no row for the stage', () => {
  const { shown } = lone();
  const drawn = timeline(shown, context(shown));
  const rows = [...drawn.querySelectorAll('.stages > li.station')];
  assert.deepEqual(rows.map((row) => row.id), ['row-found', 'row-build', 'row-shelf', 'row-ship']);
  assert.ok(!drawn.querySelector('#row-only'), 'no row for the stage');
  assert.ok(!drawn.querySelector('.fold-button, .block-rows'), 'no fold and no rows beneath a row');
  assert.ok(drawn.querySelector('.stage-pane #block-only'), 'the pane still shows the stage');
});

test('a block’s row shows its title, then its kind and, where it has one, its state as a dot and a word, as a stage’s row shows its status', () => {
  const { shown, asked } = lone();
  const drawn = timeline(shown, context(shown, { waiting: new Set([asked.key]) }));
  const facts = (id) => {
    const row = drawn.querySelector(`#row-${id}`);
    return [row.querySelector('.stage-choose .stage-title').textContent, row.querySelector('.stage-facts .kind').textContent, row.querySelector('.stage-facts .state')?.textContent ?? null];
  };
  assert.deepEqual(facts('found'), ['What it found', 'Note', null]);
  assert.deepEqual(facts('build'), ['Build the shelf', 'Task', 'Active']);
  assert.deepEqual(facts('shelf'), ['shelf', 'Question', 'Waits on you']);
  assert.deepEqual(facts('ship'), ['Ship it', 'Approval', 'Approved']);
  assert.ok(drawn.querySelector('#row-build').classList.contains('status--active'), 'the rail marks it as a stage of its state');
  assert.ok(drawn.querySelector('#row-ship').classList.contains('status--done'));
  assert.equal(blockState(block('task', 't', { title: 'T', state: 'dropped' }), context(shown))?.[0], 'Dropped');
});

test('choosing a block’s row brings the block into view in the pane and marks it as the block chosen', () => {
  const { shown } = lone();
  /** @type {[string, string | undefined][]} */
  const chosen = [];
  const drawn = timeline(shown, context(shown, { onChoose: (stageId, blockId) => chosen.push([stageId, blockId]) }));
  drawn.querySelector('#row-build .stage-choose').click();
  assert.deepEqual(chosen, [['only', 'build']]);
  const again = timeline(shown, context(shown, { chosen: { block: 'build' } }));
  assert.ok(again.querySelector('#row-build').classList.contains('is-chosen'));
  assert.equal(again.querySelector('#row-build .stage-choose').getAttribute('aria-current'), 'true');
  assert.equal(again.querySelectorAll('.station.is-chosen').length, 1);
  assert.ok(again.querySelector('.stage-pane #block-build'));
});

test('with more than one stage, the list holds a row for each stage, with the rows of its blocks beneath it', () => {
  const { note, task } = lone();
  const shown = log([stage('a', 'A', 'active', [note]), stage('b', 'B', 'planned', [task])]);
  const drawn = timeline(shown, context(shown));
  assert.deepEqual([...drawn.querySelectorAll('.stages > li.station')].map((row) => row.id), ['row-a', 'row-b']);
  assert.ok(drawn.querySelector('#row-a .block-rows [data-block="found"]'));
});

test('beneath a stage of the list, each block’s row gives its kind as a small light icon, named to a reader and on hover, and no word', () => {
  const kinds = ['note', 'question', 'approval', 'plan', 'decision', 'task', 'loft'];
  const blocks = kinds.map((kind) => (kind === 'question' ? question('shelf', ['A', 'B']) : block(kind, `b-${kind}`, { title: kind, markdown: 'x', html: '' })));
  const shown = log([stage('now', 'Now', 'active', blocks), stage('next', 'Next', 'planned', [])]);
  const rows = [...timeline(shown, context(shown)).querySelectorAll('#row-now .block-rows .block-row')];
  const names = { note: 'Note', question: 'Question', approval: 'Approval', plan: 'Plan', decision: 'Decision', task: 'Task', loft: 'Loft' };
  rows.forEach((row, index) => {
    const icon = row.querySelector('.kind-icon');
    assert.equal(icon?.getAttribute('role'), 'img');
    assert.equal(icon?.getAttribute('aria-label'), names[kinds[index]]);
    assert.equal(icon?.getAttribute('title'), names[kinds[index]]);
    assert.ok(icon?.querySelector('svg path'), 'drawn, not written');
    assert.ok(!row.querySelector('.kind'), 'no word of its kind');
  });
  assert.deepEqual(Object.keys(KIND_ICONS).sort(), [...kinds].sort(), 'one icon for each kind');
  assert.deepEqual(KIND_ICONS.loft, ['M5.75 3.5v9h5'], 'an L for a loft block');
  const css = readFileSync(new URL('../../yard-page/yard.css', import.meta.url), 'utf8');
  assert.match(css, /\.kind-icon \{[^}]*width: 14px; height: 14px; color: var\(--muted\);/);
  assert.match(css, /\.kind-icon svg \{ fill: none; stroke: currentColor; stroke-width: 1\.25;/);
});
