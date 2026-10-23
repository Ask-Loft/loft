// Every diagram the page draws expands into the page's modal, where it zooms and pans. Mermaid is
// stood in for by a stub that draws each source as an SVG holding its text.

import { forget, root } from './dom.js';

import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';

import { drawAll } from '../../yard-page/lib/diagrams.js';
import { h } from '../../yard-page/lib/dom.js';
import * as modal from '../../yard-page/lib/modal.js';
import { loftView } from '../../yard-page/views/loft/index.js';

const window = /** @type {any} */ (globalThis.window);
globalThis.requestAnimationFrame = (callback) => setTimeout(callback, 0);

/** @type {any} */ (globalThis).mermaid = {
  initialize() {},
  async render(_id, text) {
    if (text.includes('not a diagram')) throw new Error('Parse error');
    const escaped = text.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
    return { svg: `<svg viewBox="0 0 400 200"><text>${escaped}</text></svg>` };
  },
};

beforeEach(() => {
  forget();
  modal.close('diagram');
  root();
});

/** Lets the drawings asked for finish. */
async function settled() {
  for (let turn = 0; turn < 5; turn += 1) await new Promise((resolve) => setTimeout(resolve, 0));
}

/** A note block titled `title` whose Markdown holds a diagram of `source`, on the page. */
function noteWith(title, source, key = 'note-key') {
  const article = h(
    'article',
    { class: 'block block--note', dataset: { key } },
    h('header', { class: 'block-head' }, h('h4', {}, title)),
    h('div', { class: 'prose' }, h('pre', {}, h('code', { class: 'language-mermaid' }, source))),
  );
  document.body.firstElementChild.append(article);
  return article;
}

/** The dialog of the expanded diagram. */
function dialog() {
  return /** @type {any} */ (document.querySelector('dialog.modal[data-modal="diagram"]'));
}

/** The text of the diagram drawn under `at`. */
function drawnText(at) {
  return at.querySelector('svg text')?.textContent;
}

test('each diagram of a block’s Markdown shows a button, reached by the keyboard, that expands it', async () => {
  const note = noteWith('How it flows', 'flowchart LR\n  A --> B');
  drawAll(note);
  await settled();
  const button = note.querySelector('.diagram .diagram-expand');
  assert.ok(button, 'a button');
  assert.equal(button.tagName, 'BUTTON');
  assert.equal(button.getAttribute('type'), 'button');
  assert.match(button.getAttribute('aria-label'), /Expand the diagram/);
});

test('a diagram that could not be drawn shows no button to expand it', async () => {
  const note = noteWith('Broken', 'not a diagram');
  drawAll(note);
  await settled();
  assert.ok(note.querySelector('.diagram-error'), 'says it could not be drawn');
  assert.ok(!note.querySelector('.diagram-expand'));
});

/** A spec-change graph of four Feature Groups of nine changed Features each, folded. */
function folded() {
  const nodes = [{ key: 'p', kind: 'product', label: 'P-1', name: 'Reading List', parent: null, change: null }];
  for (let g = 0; g < 4; g += 1) {
    nodes.push({ key: `g${g}`, kind: 'feature_group', label: `FG-${g}`, name: `Group ${g}`, parent: 'p', change: null });
    for (let f = 0; f < 9; f += 1) nodes.push({ key: `g${g}f${f}`, kind: 'feature', label: null, name: `Feature ${g}.${f}`, parent: `g${g}`, change: 'added' });
  }
  return { nodes, decisions: [], edges: [], diff: [] };
}

/** A `loft` block of a spec-change view of `graph`, on the page. */
function specChange(graph) {
  const report = { status: { base: { revision: 'develop', added: [{ kind: 'feature', name: 'x' }], changed: [], removed: [] } }, graph };
  graph.diff = [{ key: 'x', kind: 'feature', label: null, name: 'x', change: 'added', words: { after: 'x' } }];
  const article = h(
    'article',
    { class: 'block block--loft', dataset: { key: 'loft-key' } },
    h('header', { class: 'block-head' }, h('h4', {}, 'Spec change')),
    loftView({ view: 'spec-change', report, recorded: {}, reasons: [] }),
  );
  document.body.firstElementChild.append(article);
  return article;
}

test('each spec-change diagram shows the button that expands it', async () => {
  const block = specChange(folded());
  await settled();
  assert.ok(block.querySelector('.spec-diagram .diagram-expand'));
});

test('expanding a diagram opens it in a modal over the page, named by its block, with no button of its own', async () => {
  const note = noteWith('How it flows', 'flowchart LR\n  A --> B');
  drawAll(note);
  await settled();
  note.querySelector('.diagram-expand').click();
  await settled();
  const opened = dialog();
  assert.ok(opened?.open, 'the modal is open');
  assert.ok(opened.classList.contains('modal') && opened.classList.contains('modal--diagram'));
  assert.equal(opened.querySelector('.modal-head h2').textContent, 'How it flows');
  assert.equal(drawnText(opened.querySelector('.zoom-stage')), 'flowchart LR\n  A --> B');
  assert.ok(!opened.querySelector('.diagram-expand'));
  assert.ok(document.activeElement === opened.querySelector('.zoom-stage'), 'the stage has the focus');
});

test('the expanded spec-change diagram is the one the page drew, with the Feature Group the person opened', async () => {
  const block = specChange(folded());
  await settled();
  const open = [...block.querySelectorAll('.group-button')].find((button) => button.textContent.startsWith('FG-2'));
  open.click();
  await settled();
  const onPage = drawnText(block.querySelector('.spec-diagram'));
  assert.match(onPage, /Feature 2\.0/);
  block.querySelector('.spec-diagram .diagram-expand').click();
  await settled();
  assert.equal(drawnText(dialog().querySelector('.zoom-stage')), onPage);
  assert.equal(dialog().querySelector('.modal-head h2').textContent, 'Spec change');
});

/** The scale the expanded diagram is drawn at. */
function scale() {
  const transform = dialog().querySelector('.zoom-content').style.transform;
  return Number(/scale\(([\d.e-]+)\)/.exec(transform)?.[1]);
}

/** Where the expanded diagram's corner lies. */
function corner() {
  const [, x, y] = /translate\(([-\d.e]+)px, ([-\d.e]+)px\)/.exec(dialog().querySelector('.zoom-content').style.transform) ?? [];
  return { x: Number(x), y: Number(y) };
}

/** Expands the diagram of a note, on a stage of 1000 by 600 pixels, and fits it there. */
async function expanded() {
  const note = noteWith('How it flows', 'flowchart LR\n  A --> B');
  drawAll(note);
  await settled();
  note.querySelector('.diagram-expand').click();
  await settled();
  const stage = dialog().querySelector('.zoom-stage');
  // The emulator lays nothing out, so the stage is given the size a browser would give it.
  Object.defineProperty(stage, 'clientWidth', { value: 1000 });
  Object.defineProperty(stage, 'clientHeight', { value: 600 });
  press(stage, '0');
  return { note, stage };
}

/** @param {Element} at @param {string} key */
function press(at, key) {
  const event = new window.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
  at.dispatchEvent(event);
  return event;
}

test('the expanded diagram shows only once it is fitted to the modal', async () => {
  const note = noteWith('How it flows', 'flowchart LR\n  A --> B');
  drawAll(note);
  await settled();
  note.querySelector('.diagram-expand').click();
  const stage = dialog().querySelector('.zoom-stage');
  assert.ok(!stage.classList.contains('is-fitted'), 'hidden while it is drawn');
  await settled();
  assert.ok(stage.classList.contains('is-fitted'), 'shown once fitted');
});

test('the keys + and - zoom in and out, 0 fits the whole diagram, and the arrows pan it', async () => {
  const { stage } = await expanded();
  const fitted = scale();
  assert.ok(press(stage, '+').defaultPrevented);
  assert.ok(scale() > fitted);
  press(stage, '-');
  press(stage, '-');
  assert.ok(scale() < fitted);
  press(stage, '0');
  assert.equal(scale(), fitted);
  const before = corner();
  press(stage, 'ArrowRight');
  assert.ok(corner().x < before.x);
  press(stage, 'ArrowDown');
  assert.ok(corner().y < before.y);
});

test('buttons in the modal zoom in, zoom out and fit, and the modal shows the zoom as a percentage', async () => {
  await expanded();
  const button = (label) => /** @type {any} */ (dialog().querySelector(`.zoom-tools [aria-label="${label}"]`));
  const shown = () => dialog().querySelector('.zoom-percent').textContent;
  const fitted = scale();
  assert.equal(shown(), `${Math.round(fitted * 100)}%`);
  button('Zoom in').click();
  assert.ok(scale() > fitted);
  assert.equal(shown(), `${Math.round(scale() * 100)}%`);
  button('Zoom out').click();
  button('Zoom out').click();
  assert.ok(scale() < fitted);
  button('Fit the whole diagram').click();
  assert.equal(scale(), fitted);
});

/** @param {Element} at @param {Record<string, any>} init */
function wheel(at, init) {
  const event = new window.WheelEvent('wheel', { bubbles: true, cancelable: true, ...init });
  // The emulator's wheel keeps only its deltas; the rest are set as a browser would give them.
  for (const [name, value] of Object.entries({ clientX: 0, clientY: 0, ctrlKey: false, metaKey: false, ...init })) {
    Object.defineProperty(event, name, { value });
  }
  at.dispatchEvent(event);
  return event;
}

test('the wheel with Ctrl or ⌘ held zooms, the wheel alone pans, and neither scrolls the modal or the page', async () => {
  const { stage } = await expanded();
  const fitted = scale();
  assert.ok(wheel(stage, { deltaY: -100, ctrlKey: true }).defaultPrevented);
  assert.ok(scale() > fitted);
  const zoomed = scale();
  wheel(stage, { deltaY: 100, metaKey: true });
  assert.ok(scale() < zoomed);
  const before = corner();
  const panned = wheel(stage, { deltaX: 30, deltaY: 50 });
  assert.ok(panned.defaultPrevented);
  assert.deepEqual(corner(), { x: before.x - 30, y: before.y - 50 });
});

test('closing the expanded diagram gives the focus back to its button, even after the page drew the block again', async () => {
  const { note } = await expanded();
  // The page draws its blocks afresh at each change, while the modal stays open.
  note.remove();
  const again = noteWith('How it flows', 'flowchart LR\n  A --> B');
  drawAll(again);
  await settled();
  dialog().querySelector('.modal-close').click();
  assert.ok(document.activeElement === again.querySelector('.diagram-expand'), 'the button drawn again has the focus');
});

test('closing the expanded diagram gives the focus back to the button that expanded it', async () => {
  const { note } = await expanded();
  dialog().querySelector('.modal-close').click();
  assert.ok(document.activeElement === note.querySelector('.diagram-expand'), 'the button has the focus');
});
