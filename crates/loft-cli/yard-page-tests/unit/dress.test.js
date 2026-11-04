// The page's dress, read from its stylesheet: the face of the system's that stands in for Inter
// until it loads, and the panel of what waits drawn as what floats over the page.

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

const css = readFileSync(new URL('../../yard-page/yard.css', import.meta.url), 'utf8');

/** The declarations of the first rule whose selector is exactly `selector`. */
function rule(selector) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const found = css.match(new RegExp(`(?:^|\\n)${escaped}\\s*\\{([^}]*)\\}`));
  assert.ok(found, `the stylesheet holds ${selector}`);
  return found[1];
}

test('the face that stands in for Inter is the system’s own, and the page loads nothing for it', () => {
  const faces = [...css.matchAll(/@font-face\s*\{([^}]*)\}/g)].map((m) => m[1]).filter((face) => /font-family:\s*"Inter Fallback"/.test(face));
  assert.equal(faces.length, 2, 'a regular and a bold face stand in for Inter');
  for (const face of faces) {
    const src = face.match(/src:([^;]*);/)?.[1] ?? '';
    assert.ok(/local\(/.test(src), 'the face names the system’s fonts');
    assert.ok(!/url\(/.test(src), 'the face loads no file');
    assert.match(face, /size-adjust:\s*[\d.]+%/);
    assert.match(face, /ascent-override:\s*[\d.]+%/);
    assert.match(face, /descent-override:\s*[\d.]+%/);
  }
  const font = css.match(/--font:\s*([^;]*);/)?.[1] ?? '';
  assert.ok(font.indexOf('"Inter"') < font.indexOf('"Inter Fallback"'), 'Inter comes first, its stand-in after it');
});

test('the panel of what waits floats over the page: an ink border and a hard shadow down and to the right', () => {
  const modal = rule('.modal[open]');
  assert.match(modal, /border:\s*1px solid var\(--ink\)/);
  assert.match(modal, /box-shadow:\s*var\(--float\)/);
  const float = css.match(/--float:\s*(\d+)px (\d+)px 0 var\(--shadow\)/);
  assert.ok(float && Number(float[1]) > 0 && Number(float[2]) > 0, 'the shadow lies down and to the right');
  assert.ok(!/box-shadow/.test(rule('.modal--docked[open]')), 'docked, the panel keeps the shadow it floats with');
});

test('only what floats over the page carries a hard shadow', () => {
  const offsets = [...css.matchAll(/([^{}\n][^{}]*)\{[^}]*box-shadow:\s*(-?[1-9]\d*px -?\d+px 0 [^;]*);/g)].map((m) => m[1].trim());
  assert.deepEqual(offsets, [], 'no rule draws its own offset shadow');
});

test('hairlines, not borders, divide the head from the panes and the panes from each other', () => {
  assert.match(rule('.page-head'), /border-bottom:\s*1px solid var\(--line\)/);
  assert.match(rule('.splitter'), /background:\s*var\(--line\)/);
  assert.ok(!/border:/.test(rule('.page-head')) && !/border:/.test(rule('.splitter')), 'neither is boxed');
});
