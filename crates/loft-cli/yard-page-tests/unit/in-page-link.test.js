// A link of rendered Markdown to a place within the page, such as a footnote's, followed without
// leaving the Yard Log the page shows.

import './dom.js';

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { html, inline } from '../../yard-page/lib/dom.js';

/** A block's Markdown as `loft` renders `Text[^1]` with its footnote. */
const FOOTNOTED = `<p>Text<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>why <a href="#fnref-1" class="footnote-backref" data-footnote-backref aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>`;

/**
 * Draws each of `drawn` in the page, and gives where each element brought into view lies, as
 * the number of its block and its id: plain words, since comparing elements themselves would
 * walk the whole emulated page.
 */
function shown(...drawn) {
  document.body.replaceChildren(...drawn);
  /** @type {string[]} */
  const brought = [];
  Element.prototype.scrollIntoView = function scrollIntoView() {
    brought.push(`${drawn.indexOf(this.closest('.prose'))}#${this.id}`);
  };
  return brought;
}

const ROUTE = '#tree=%2Ftree&branch=main';

/** Follows `link` as a click does, and gives whether the page kept the browser from following it. */
function follow(link) {
  const click = new Event('click', { bubbles: true, cancelable: true });
  link.dispatchEvent(click);
  return click.defaultPrevented;
}

test('following a footnote’s reference in a note brings the note’s footnote into view', () => {
  const block = html(FOOTNOTED);
  const brought = shown(block);
  block.querySelector('a[href="#fn-1"]').click();
  assert.deepEqual(brought, ['0#fn-1']);
});

test('following a footnote’s back link brings its reference into view', () => {
  const block = html(FOOTNOTED);
  const brought = shown(block);
  block.querySelector('a[href="#fnref-1"]').click();
  assert.deepEqual(brought, ['0#fnref-1']);
});

test('following a link to a place within the page in a note leaves the address as it was', () => {
  location.hash = ROUTE;
  const block = html(FOOTNOTED);
  shown(block);
  assert.ok(follow(block.querySelector('a[href="#fn-1"]')));
  assert.equal(location.hash, ROUTE);
});

test('following a link to a place within the page in a node’s name leaves the address as it was', () => {
  location.hash = ROUTE;
  const name = inline('See <a href="#notes">the notes</a>');
  shown(name);
  assert.ok(follow(name.querySelector('a')));
  assert.equal(location.hash, ROUTE);
});

test('where two blocks each hold a footnote, the reference in each brings its own block’s footnote into view', () => {
  const [first, second] = [html(FOOTNOTED), html(FOOTNOTED)];
  const brought = shown(first, second);
  second.querySelector('a[href="#fn-1"]').click();
  first.querySelector('a[href="#fn-1"]').click();
  assert.deepEqual(brought, ['1#fn-1', '0#fn-1']);
});

test('following the reference of a footnote whose name holds a letter outside ASCII brings that footnote into view', () => {
  // As `loft` renders `Text[^café]`: the name percent-encoded in the id as in the address.
  const block = html(FOOTNOTED.replaceAll('fn-1', 'fn-caf%C3%A9').replaceAll('fnref-1', 'fnref-caf%C3%A9'));
  const brought = shown(block);
  block.querySelector('a[data-footnote-ref]').click();
  assert.deepEqual(brought, ['0#fn-caf%C3%A9']);
});

test('following a link to a place within the page from the keyboard moves the focus to that place', () => {
  const block = html(FOOTNOTED);
  shown(block);
  block.querySelector('a[href="#fn-1"]').click();
  assert.equal(document.activeElement?.id, 'fn-1');
  block.querySelector('a[href="#fnref-1"]').click();
  assert.equal(document.activeElement?.id, 'fnref-1');
  assert.equal(block.querySelector('#fnref-1').getAttribute('tabindex'), null, 'a link stays in the order of the keyboard');
});

test('following a link in a note to the address of a Yard Log is left to the browser', () => {
  const block = html('<p><a href="#tree=%2Ftree&amp;branch=main&amp;block=d1">the decision</a></p>');
  shown(block);
  assert.equal(follow(block.querySelector('a')), false);
});

test('opening a footnote’s reference in a new tab opens the Yard page on the Yard Log it shows', () => {
  location.hash = ROUTE;
  const block = html(FOOTNOTED);
  const brought = shown(block);
  const reference = block.querySelector('a[href="#fn-1"]');
  for (const type of ['pointerdown', 'focus']) {
    reference.setAttribute('href', '#fn-1');
    reference.dispatchEvent(new Event(type));
    assert.equal(reference.getAttribute('href'), ROUTE, type);
  }
  reference.click();
  assert.deepEqual(brought, ['0#fn-1'], 'a click still brings the footnote into view');
});
