// The page's modal, which every view that opens one opens.

import { forget, root } from './dom.js';

import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';

import { h } from '../../yard-page/lib/dom.js';
import * as modal from '../../yard-page/lib/modal.js';

beforeEach(() => {
  forget();
  root();
});

/** The dialog the modal `name` is drawn in. */
function dialogOf(name) {
  return /** @type {any} */ (document.querySelector(`dialog.modal[data-modal="${name}"]`));
}

test('a modal opens over the page with its title, its content and a button that closes it', () => {
  modal.show('shelf', { title: 'The shelf', content: h('p', { class: 'inside' }, 'Oak') });
  const dialog = dialogOf('shelf');
  assert.ok(dialog.open);
  assert.equal(dialog.querySelector('.modal-head h2').textContent, 'The shelf');
  assert.equal(dialog.querySelector('.modal-body .inside').textContent, 'Oak');
  assert.equal(dialog.getAttribute('aria-labelledby'), dialog.querySelector('.modal-head h2').id);
  assert.ok(modal.isOpen('shelf'));
  dialog.querySelector('.modal-close').click();
  assert.ok(!dialog.open);
  assert.ok(!modal.isOpen('shelf'));
});

test('a modal closes on Escape, and tells whoever opened it that it closed', () => {
  let closed = 0;
  modal.show('shelf', { title: 'The shelf', content: 'Oak', onClose: () => closed++ });
  const dialog = dialogOf('shelf');
  const cancel = new Event('cancel', { cancelable: true });
  dialog.dispatchEvent(cancel);
  assert.ok(!dialog.open);
  assert.equal(closed, 1);
});

test('showing an open modal again draws its new content in the same dialog', () => {
  modal.show('shelf', { title: 'One', content: 'Oak' });
  const dialog = dialogOf('shelf');
  modal.show('shelf', { title: 'Two', content: 'Ash' });
  assert.ok(dialogOf('shelf') === dialog, 'the same dialog');
  assert.equal(dialog.querySelector('.modal-head h2').textContent, 'Two');
  assert.equal(dialog.querySelector('.modal-body').textContent, 'Ash');
});

test('a change drawn while the modal is open keeps its scroll position and the field that has focus', () => {
  // The field drawn again holds what was typed, as the page's fields hold what the browser kept.
  const field = () => {
    const textarea = /** @type {any} */ (h('textarea', { id: 'why' }));
    textarea.value = 'Too dark';
    return textarea;
  };
  modal.show('shelf', { title: 'The shelf', content: [h('p', {}, 'Long'), field()] });
  const dialog = dialogOf('shelf');
  const scrolled = dialog.querySelector('.modal-body');
  scrolled.scrollTop = 120;
  const before = dialog.querySelector('#why');
  before.focus();
  before.setSelectionRange(2, 5);
  modal.update('shelf', { title: 'The shelf', content: [h('p', {}, 'Longer'), field()] });
  const after = dialog.querySelector('#why');
  assert.ok(after !== before, 'the content was drawn again');
  assert.equal(dialog.querySelector('.modal-body').scrollTop, 120);
  assert.ok(document.activeElement === after, 'the focus is where it should be');
  assert.equal(after.selectionStart, 2);
  assert.equal(after.selectionEnd, 5);
});

test('updating a modal that is closed draws nothing and opens nothing', () => {
  modal.show('shelf', { title: 'The shelf', content: 'Oak' });
  modal.close('shelf');
  modal.update('shelf', { title: 'The shelf', content: 'Ash' });
  assert.ok(!dialogOf('shelf').open);
  assert.equal(dialogOf('shelf').querySelector('.modal-body').textContent, 'Oak');
});

test('closing the modal gives the focus back to what held it before it opened', () => {
  const opener = h('button', { type: 'button' }, 'Open');
  document.body.append(opener);
  opener.focus();
  modal.show('shelf', { title: 'The shelf', content: h('input', { id: 'inside' }) });
  /** @type {any} */ (document.getElementById('inside')).focus();
  modal.close('shelf');
  assert.ok(document.activeElement === opener, 'the focus is where it should be');
});

test('a modal whose dialog left the document is drawn afresh the next time it is shown', () => {
  modal.show('shelf', { title: 'The shelf', content: 'Oak' });
  root();
  assert.ok(!modal.isOpen('shelf'));
  modal.show('shelf', { title: 'The shelf', content: 'Ash' });
  assert.ok(dialogOf('shelf').isConnected);
  assert.ok(dialogOf('shelf').open);
});
