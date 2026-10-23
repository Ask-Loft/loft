// The page's two colour schemes, read from its stylesheet: the light one on `:root`, the dark
// one under `prefers-color-scheme: dark`.

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

const css = readFileSync(new URL('../../yard-page/yard.css', import.meta.url), 'utf8');

/** The colour tokens a block of declarations sets, by name. */
function tokens(block) {
  return new Map([...block.matchAll(/--([a-z0-9-]+):\s*(#[0-9a-f]{6})\s*;/gi)].map((m) => [m[1], m[2].toLowerCase()]));
}

/** @param {RegExp} pattern */
function block(pattern) {
  const found = css.match(pattern);
  assert.ok(found, `the stylesheet holds ${pattern}`);
  return tokens(found[1]);
}

const light = block(/^:root\s*\{([^}]*)\}/m);
const dark = block(/@media \(prefers-color-scheme: dark\)\s*\{\s*:root\s*\{([^}]*)\}/);

/** WCAG relative luminance of `#rrggbb`. */
function luminance(hex) {
  const n = Number.parseInt(hex.slice(1), 16);
  return [n >> 16, (n >> 8) & 255, n & 255]
    .map((c) => c / 255)
    .map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4))
    .reduce((sum, c, i) => sum + c * [0.2126, 0.7152, 0.0722][i], 0);
}

function contrast(a, b) {
  const [high, low] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (high + 0.05) / (low + 0.05);
}

/** Every colour the page gives text, and every ground it draws text on. */
const TEXTS = ['ink', 'body', 'muted', 'signal', 'done', 'wait', 'danger', 'info', 'plan', 'spec', 'new', 'drop'];
const GROUNDS = ['canvas', 'surface', 'surface-2', 'mist', 'signal-soft', 'done-soft', 'wait-soft', 'danger-soft', 'info-soft', 'plan-soft', 'spec-soft'];

test('each colour the page gives text holds 4.5 to 1 against each ground, in either scheme', () => {
  for (const [scheme, colours] of [['light', light], ['dark', dark]]) {
    for (const text of TEXTS) {
      for (const ground of GROUNDS) {
        const [fore, back] = [colours.get(text), colours.get(ground)];
        assert.ok(fore && back, `the ${scheme} scheme sets --${text} and --${ground}`);
        const ratio = contrast(fore, back);
        assert.ok(ratio >= 4.5, `${scheme}: --${text} ${fore} on --${ground} ${back} is ${ratio.toFixed(2)} to 1`);
      }
    }
    const ratio = contrast(colours.get('on-button'), colours.get('button'));
    assert.ok(ratio >= 4.5, `${scheme}: a button's text is ${ratio.toFixed(2)} to 1`);
  }
});

test('the dark scheme sets every colour the light scheme sets, each its own', () => {
  assert.ok(light.size >= TEXTS.length + GROUNDS.length);
  for (const [name, value] of light) {
    assert.ok(dark.has(name), `the dark scheme sets --${name}`);
    if (TEXTS.includes(name) || GROUNDS.includes(name)) assert.notEqual(dark.get(name), value, `--${name} differs in the dark`);
  }
});
