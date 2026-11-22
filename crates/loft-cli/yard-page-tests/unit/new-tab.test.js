// A link that rendered Markdown holds to a web address, opened in a new tab.

import './dom.js';

import { readFileSync } from 'node:fs';

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { html, inline } from '../../yard-page/lib/dom.js';

test('a link to a web address in a block’s Markdown opens in a new tab', () => {
  const block = html('<p><a href="https://example.com/a">a</a> and <a href="HTTP://example.com/b">b</a></p>');
  for (const link of block.querySelectorAll('a')) assert.equal(link.getAttribute('target'), '_blank', link.href);
});

test('a link to a web address in a node’s name opens in a new tab', () => {
  const name = inline('See <a href="https://example.com/docs">the docs</a>');
  assert.equal(name.querySelector('a').getAttribute('target'), '_blank');
});

test('a page that a link of rendered Markdown opens is given no hold on the Yard page', () => {
  for (const drawn of [html('<p><a href="https://example.com/a">a</a></p>'), inline('<a href="https://example.com/b">b</a>')]) {
    assert.equal(drawn.querySelector('a').getAttribute('rel'), 'noopener noreferrer');
  }
});

test('a mailto link of rendered Markdown opens no new tab', () => {
  const link = html('<p><a href="mailto:someone@example.com">mail</a></p>').querySelector('a');
  assert.equal(link.getAttribute('target'), null);
  assert.equal(link.getAttribute('rel'), null);
});

test('a link of rendered Markdown within the page opens no new tab', () => {
  const link = html('<p><a href="#fn-1">1</a></p>').querySelector('a');
  assert.equal(link.getAttribute('target'), null);
});

test('a screen reader hears that a link of rendered Markdown opens in a new tab', () => {
  const shell = readFileSync(new URL('../../yard-page/index.html', import.meta.url), 'utf8');
  assert.match(shell, /<span id="new-tab-note" hidden>opens in a new tab<\/span>/);
  for (const drawn of [html('<p><a href="https://example.com/a">a</a></p>'), inline('<a href="https://example.com/b">b</a>')]) {
    assert.equal(drawn.querySelector('a').getAttribute('aria-describedby'), 'new-tab-note');
  }
  assert.equal(html('<p><a href="mailto:a@example.com">a</a></p>').querySelector('a').getAttribute('aria-describedby'), null);
});

test('a link of rendered Markdown that opens in a new tab holds its own words alone', () => {
  const link = html('<p>See <a href="https://example.com/plan">https://example.com/plan</a> for it.</p>').querySelector('p');
  assert.equal(link.textContent, 'See https://example.com/plan for it.');
});

test('a link to the address of a Yard Log on this page opens no new tab and names that Yard Log', () => {
  const route = '#tree=%2Ftree&branch=main&block=d1';
  const link = html(`<p><a href="${location.origin}${location.pathname}${route}">the decision</a></p>`).querySelector('a');
  assert.equal(link.getAttribute('href'), route);
  assert.equal(link.getAttribute('target'), null);
  const elsewhere = html('<p><a href="http://127.0.0.1:9999/#tree=%2Ftree&branch=main">another page</a></p>').querySelector('a');
  assert.equal(elsewhere.getAttribute('target'), '_blank');
});
