// A colour written in a code span of rendered Markdown, drawn on that colour.

import { root } from './dom.js';

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { html, inkOn, inline } from '../../yard-page/lib/dom.js';

/** `colour` as the browser writes it back once set as a background. */
function asSet(colour) {
  const probe = root();
  probe.style.backgroundColor = colour;
  return probe.style.backgroundColor;
}

/** WCAG contrast between two opaque `#rrggbb` colours. */
function contrast(a, b) {
  const luminance = (hex) => {
    const n = Number.parseInt(hex.slice(1), 16);
    return [n >> 16, (n >> 8) & 255, n & 255]
      .map((c) => c / 255)
      .map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4))
      .reduce((sum, c, i) => sum + c * [0.2126, 0.7152, 0.0722][i], 0);
  };
  const [high, low] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (high + 0.05) / (low + 0.05);
}

test('a code span holding a colour such as #62359c is drawn on that colour', () => {
  const block = html('<p>Plan mode is <code>#62359c</code>, done is <code>#0A756C</code>.</p>');
  const [plan, done] = block.querySelectorAll('code');
  assert.equal(plan.style.backgroundColor, asSet('#62359c'));
  assert.ok(plan.classList.contains('colour'));
  assert.equal(done.style.backgroundColor, asSet('#0A756C'));
  const name = inline('The <code>#fff</code> ground');
  assert.equal(name.querySelector('code').style.backgroundColor, asSet('#fff'));
});

test('the text of an opaque colour’s code span holds 4.5 to 1 against that colour', () => {
  const sweep = ['#000000', '#ffffff', '#767676', '#777777', '#888888', '#62359c', '#0a756c', '#a8480c', '#aee2fd', '#56d3da', '#eb6834', '#1baf7a', '#ffff00', '#0000ff', '#ff0000'];
  for (const colour of sweep) {
    const ink = inkOn(colour);
    assert.ok(contrast(ink, colour) >= 4.5, `${ink} on ${colour} is ${contrast(ink, colour).toFixed(2)} to 1`);
  }
  for (let grey = 0; grey <= 255; grey += 1) {
    const colour = `#${grey.toString(16).padStart(2, '0').repeat(3)}`;
    assert.ok(contrast(inkOn(colour), colour) >= 4.5, `on ${colour}`);
  }
  const span = html('<p><code>#62359c</code></p>').querySelector('code');
  assert.equal(span.style.color, asSet(inkOn('#62359c')));
});

test('a code span holding more than a colour, and a colour in a fenced code block, keep the page’s code colours', () => {
  const block = html('<p><code>colour #62359c</code> <code>#62359g</code> <code>#12345</code></p><pre><code>#62359c</code></pre>');
  for (const code of block.querySelectorAll('code')) {
    assert.equal(code.getAttribute('style'), null, code.textContent);
    assert.ok(!code.classList.contains('colour'), code.textContent);
  }
});
