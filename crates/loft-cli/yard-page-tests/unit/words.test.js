// The words a node had before a change and has after it, compared word by word.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { MOST_PAIRS, compareWords } from '../../yard-page/model/words.js';

/** The runs as text, each removed run in [-…-] and each added one in {+…+}. */
function marked(runs) {
  return runs.map(({ text, mark }) => (mark === 'removed' ? `[-${text}-]` : mark === 'added' ? `{+${text}+}` : text)).join('');
}

test('the words a change added are marked added, and the rest kept as they were', () => {
  const runs = compareWords('An edit made through the link is refused.', 'An edit made through the link is refused, and the link says so.');
  assert.equal(marked(runs), 'An edit made through the link is [-refused.-]{+refused, and the link says so.+}');
});

test('the words a change removed are marked removed', () => {
  assert.equal(marked(compareWords('The shelf holds every book.', 'The shelf holds a book.')), 'The shelf holds [-every -]{+a +}book.');
});

test('the same words before and after give one run kept as it was', () => {
  assert.deepEqual(compareWords('Keep a change log', 'Keep a change log'), [{ text: 'Keep a change log', mark: 'same' }]);
});

test('nothing before marks every word added, and nothing after every word removed', () => {
  assert.deepEqual(compareWords('', 'A new one'), [{ text: 'A new one', mark: 'added' }]);
  assert.deepEqual(compareWords('An old one', ''), [{ text: 'An old one', mark: 'removed' }]);
});

test('the runs joined give the text after back, with the words removed left out', () => {
  const before = 'Keep a change log\n\nAn append-only log.';
  const after = 'Keep a change log\n\nAn append-only log, kept a year.';
  const runs = compareWords(before, after);
  assert.equal(runs.filter((run) => run.mark !== 'removed').map((run) => run.text).join(''), after);
  assert.equal(runs.filter((run) => run.mark !== 'added').map((run) => run.text).join(''), before);
});

test('texts too long to compare word by word count as the whole before removed and the whole after added', () => {
  const length = Math.ceil(Math.sqrt(MOST_PAIRS)) + 1;
  const before = Array.from({ length }, (_, n) => `a${n}`).join(' ');
  const after = Array.from({ length }, (_, n) => `b${n}`).join(' ');
  assert.deepEqual(
    compareWords(before, after).map((run) => run.mark),
    ['removed', 'added'],
  );
});
