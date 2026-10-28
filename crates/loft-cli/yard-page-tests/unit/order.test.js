// Creation, update and presentation order are independent of the latest version's timestamp.
import { forget, root } from './dom.js';
import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';
import { blocksByKey, decisionsByQuestion, openSubmissions, waitingBlocks } from '../../yard-page/model/log.js';
import { submissionPanel } from '../../yard-page/views/answering.js';
import { card } from '../../yard-page/views/blocks.js';
import { timeline } from '../../yard-page/views/timeline.js';
import { act, block, log, open, question, stage } from './fixtures.js';

beforeEach(forget);

const context = (shown, order = 'oldest') => ({
  blocks: blocksByKey(shown), decisions: decisionsByQuestion(shown),
  marked: new Set(), waiting: new Set(), order, now: Date.parse('2026-01-01T12:00:00Z'),
  modes: [{ mode: 'plan', started: '2026-01-01T09:00:00Z', ended: '2026-01-01T11:00:00Z' }],
});

test('a revised card labels creation and update independently even within the same second', () => {
  for (const updated of ['2026-01-01T10:00:00Z', '2026-01-01T10:30:00Z']) {
    const note = block('note', 'revised', { markdown: 'now' });
    const first = note.version;
    note.version = { ...first, key: 'new', number: 2, act: act(updated, 2) };
    note.versions = [first, note.version];
    note.versionCount = 2;
    const meta = card(note, context(log([stage('s', 'S', 'active', [note])]))).querySelector('.block-meta');
    assert.match(meta.textContent, /Created/);
    assert.match(meta.textContent, /Updated/);
    assert.deepEqual([...meta.querySelectorAll('time')].map(t => t.getAttribute('datetime')),
      [first.act.at, updated]);
    assert.equal(meta.querySelectorAll('details.versions').length, 1);
  }
});

test('updating a block preserves its position through mode grouping and reversing stages', () => {
  const a = block('note', 'a', { markdown: 'A' });
  const b = block('note', 'b', { markdown: 'B' });
  const c = block('note', 'c', { markdown: 'C' });
  const first = stage('first', 'First', 'active', [a, b]);
  const second = stage('second', 'Second', 'active', [c]);
  const shown = log([first, second]);
  for (const updated of [false, true]) {
    if (updated) a.version = { ...a.version, act: act('2026-01-01T11:30:00Z', 5), number: 2 };
    for (const order of ['oldest', 'newest']) {
      const drawn = timeline(shown, context(shown, order));
      assert.deepEqual([...drawn.querySelectorAll('article.block')].map(e => e.id),
        order === 'newest' ? ['block-c', 'block-a', 'block-b'] : ['block-a', 'block-b', 'block-c']);
    }
  }
});

test('same second submissions use local sending arrivals with revision and key fallbacks', () => {
  const group = (key, revision, arrived) => ({ agent: { session: key }, open: [{
    key, state: 'open', blocks: [], sent: act('2026-01-01T10:00:00Z', revision),
    ...(arrived === undefined ? {} : { sentReached: arrived }),
  }] });
  for (const [groups, expected] of [
    [[group('later', 1, 9), group('earlier', 80, 7)], ['earlier', 'later']],
    [[group('later', 2), group('earlier', 1)], ['earlier', 'later']],
    [[group('b', 2, 9), group('a', 1, 9)], ['a', 'b']],
  ]) {
    const shown = { submissions: groups };
    const original = JSON.stringify(shown);
    assert.deepEqual(openSubmissions(shown).map(s => s.key), expected);
    assert.equal(JSON.stringify(shown), original);
  }
});

test('mixed waiting forms retain submission order and share one question send action through rerenders', async () => {
  const plan = block('plan', 'plan', { title: 'Plan', markdown: 'x', html: '<p>x</p>' });
  const first = question('first', ['Yes', 'No']);
  const approval = block('approval', 'approval', { title: 'Approve', markdown: 'x', html: '<p>x</p>' });
  const second = question('second', ['Yes', 'No']);
  const blocks = [plan, first, approval, second];
  const group = open(blocks, '2026-01-01T10:00:00Z');
  const submission = group.open[0];
  const shown = log([stage('s', 'S', 'active', blocks)], { submissions: [group] });
  let finish;
  const sent = new Promise(resolve => { finish = resolve; });
  const where = { tree: '/work/shelf', branch: 'feat/shelf', refresh: () => finish(), notify: () => {} };
  const draw = () => submissionPanel(submission, waitingBlocks(submission, blocksByKey(shown)), where);
  const keys = panel => [...panel.querySelectorAll('.question-form, .decision-form')].map(e => e.dataset.key);
  let panel = draw();
  root().append(panel);
  assert.deepEqual(keys(panel), blocks.map(b => b.key));
  panel.querySelector(`.question-form[data-key="${first.key}"] input[value="Yes"]`).click();
  plan.outcome = { verdict: 'approved' };
  panel.remove();
  panel = draw();
  root().append(panel);
  assert.deepEqual(keys(panel), [first.key, approval.key, second.key]);
  assert.equal(panel.querySelector(`.question-form[data-key="${first.key}"] input[value="Yes"]`).checked, true);
  panel.querySelector(`.question-form[data-key="${second.key}"] input[value="No"]`).click();
  assert.equal(panel.querySelectorAll(`#send-${submission.key}`).length, 1);
  const fetched = globalThis.fetch;
  let posted;
  globalThis.fetch = async (_, init) => {
    posted = JSON.parse(String(init.body));
    return new Response(JSON.stringify({ outcome: 'answered', errors: [] }), { status: 200 });
  };
  try {
    panel.querySelector(`#send-${submission.key}`).click();
    await sent;
    assert.deepEqual(posted.answers.map(a => a.block), [first.key, second.key]);
  } finally { globalThis.fetch = fetched; }
});
