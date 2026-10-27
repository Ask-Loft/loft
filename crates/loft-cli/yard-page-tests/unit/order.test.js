// Creation, update and presentation order are independent of the latest version's timestamp.
import './dom.js';
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { blocksByKey, decisionsByQuestion } from '../../yard-page/model/log.js';
import { card } from '../../yard-page/views/blocks.js';
import { timeline } from '../../yard-page/views/timeline.js';
import { act, block, log, stage } from './fixtures.js';

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
