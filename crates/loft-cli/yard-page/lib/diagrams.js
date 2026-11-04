// Diagrams: each code block of a block's Markdown marked `mermaid`, and each diagram the page
// builds, drawn by the copy of Mermaid the page server serves with the page. Mermaid is loaded
// the first time a diagram is drawn, in its strict mode, which draws no HTML the source holds.
// It draws in the page's own design for the scheme the browser asks for: square nodes on the
// sidebar's ground with hairline borders, the page's face at 13 pixels, muted edges, and groups
// framed by a hairline on the page's ground; and each diagram on the page is drawn again when the
// browser changes its scheme. A flowchart of a block's Markdown is drawn by the page itself, over
// ELK (`flowchart.js`), in classes the stylesheet colours, so a change of scheme recolours it
// without laying it out again; so is a diagram the page builds itself, given as a drawing that
// draws its own SVG. A diagram the page draws keeps its own size, scrolling sideways where it is
// wider than its block, and brings a node's neighbours forward under the pointer. Each diagram
// drawn has a button that expands it into the page's modal, where it zooms and pans.

import { h } from './dom.js';
import { emphasis } from './emphasis.js';
import { FLOWCHART, drawFlowchart } from './flowchart.js';
import * as modal from './modal.js';
import { zoomStage } from './zoom.js';

/** The colours of the page's light scheme, for a page whose stylesheet sets none. */
const LIGHT = {
  font: '"Inter", ui-sans-serif, system-ui, sans-serif',
  surface: '#ffffff',
  surface2: '#f3f7fa',
  side: '#f6f9fb',
  canvas: '#ffffff',
  mist: '#e5f6ff',
  ink: '#0e141d',
  muted: '#5c646f',
  line: '#dbe2e9',
  rail: '#aab2ba',
  done: '#0a756c',
  doneSoft: '#e3f4f1',
  wait: '#a8480c',
  waitSoft: '#fbeee4',
  danger: '#b42318',
  dangerSoft: '#fde8e7',
};

/** @typedef {typeof LIGHT} Colours */

/** @returns {'dark' | 'light'} the scheme the browser asks for. */
function scheme() {
  return globalThis.matchMedia?.('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

/**
 * The colours a diagram is drawn in: the page's tokens, as its stylesheet sets them for the
 * scheme the browser asks for.
 * @returns {Colours}
 */
export function colours() {
  const style = globalThis.getComputedStyle?.(document.documentElement);
  /** @param {string} token */
  const read = (token) => style?.getPropertyValue(`--${token}`).trim() || '';
  const kebab = (/** @type {string} */ name) => name.replace(/[A-Z0-9]/g, (letter) => `-${letter.toLowerCase()}`);
  return /** @type {Colours} */ (Object.fromEntries(Object.entries(LIGHT).map(([name, fallback]) => [name, read(kebab(name)) || fallback])));
}

/**
 * Mermaid's theme in the page's design: nodes on the sidebar's ground with hairline borders, the
 * page's face at 13 pixels, muted edges, and groups and edge labels on the page's ground.
 * @param {Colours} c
 */
export function theme(c) {
  return {
    darkMode: scheme() === 'dark',
    fontFamily: c.font,
    fontSize: '13px',
    background: c.canvas,
    primaryColor: c.side,
    primaryTextColor: c.ink,
    primaryBorderColor: c.rail,
    secondaryColor: c.mist,
    secondaryTextColor: c.ink,
    secondaryBorderColor: c.rail,
    tertiaryColor: c.canvas,
    tertiaryTextColor: c.ink,
    tertiaryBorderColor: c.line,
    mainBkg: c.side,
    nodeBorder: c.rail,
    lineColor: c.muted,
    textColor: c.ink,
    titleColor: c.ink,
    clusterBkg: c.canvas,
    clusterBorder: c.line,
    edgeLabelBackground: c.canvas,
    noteBkgColor: c.waitSoft,
    noteTextColor: c.ink,
    noteBorderColor: c.wait,
  };
}

let loading = null;

/** The scheme Mermaid was last set up for. */
let configured = null;

/** @returns {Promise<any>} Mermaid, set up for the scheme the browser now asks for. */
async function mermaid() {
  if (!loading) {
    loading = new Promise((resolve, reject) => {
      const found = /** @type {any} */ (globalThis).mermaid;
      if (found) {
        resolve(found);
        return;
      }
      const script = document.createElement('script');
      script.src = 'vendor/mermaid.min.js';
      script.onload = () => resolve(/** @type {any} */ (globalThis).mermaid);
      script.onerror = () => reject(new Error('the diagram library did not load'));
      document.head.append(script);
    });
  }
  const library = await loading;
  if (configured !== scheme()) {
    configured = scheme();
    library.initialize({ startOnLoad: false, securityLevel: 'strict', theme: 'base', themeVariables: theme(colours()) });
  }
  return library;
}

let counter = 0;

/** Each source drawn in each scheme, with what it was drawn as, so that a page drawn again draws at once. */
const drawn = new Map();

/** The most drawings `drawn` keeps; past it the one kept longest is forgotten. */
const KEPT = 200;

/** Each diagram drawn, with its source, so that a change of scheme draws it again. */
const shown = new Map();

/** The latest drawing asked of each diagram, so that an earlier one finishing late is dropped. */
const turns = new WeakMap();

/** The targets of the diagrams drawn expanded, in the modal, which have no button to expand. */
const EXPANDED = new WeakSet();

/**
 * A diagram the page builds and draws itself: `draw` gives its SVG's markup, the same whatever
 * the scheme, under `key`, for its block or, `expanded`, for the modal; `choose`, where given, is
 * told the key of a node the person chooses.
 * @typedef {{ key: string, draw: (id: string, expanded: boolean) => Promise<string>, choose?: (key: string) => void }} Drawing
 */

/**
 * Shows `svg` in `target`: a diagram the page drew keeps its own size within a box that scrolls
 * sideways, unless it is drawn expanded, and takes the hover layer.
 * @param {HTMLElement} target
 * @param {string} svg
 */
function show(target, svg) {
  target.innerHTML = svg;
  const own = target.querySelector(':scope > svg.flow');
  if (own && !EXPANDED.has(target)) {
    const scroll = document.createElement('div');
    scroll.className = 'diagram-scroll';
    own.replaceWith(scroll);
    scroll.append(own);
  }
  if (own) emphasis(target);
  target.classList.add('is-drawn');
  expandable(target);
}

/**
 * Draws `source` into `target`: Mermaid's source, the source that `source` makes from the
 * colours of the scheme, or a drawing; with a button that expands it. Where it cannot be drawn,
 * shows its source with a line saying it could not be drawn, and no button.
 * @param {HTMLElement} target
 * @param {string | ((colours: Colours) => string) | Drawing} source
 */
export async function draw(target, source) {
  shown.set(target, source);
  const turn = (turns.get(target) ?? 0) + 1;
  turns.set(target, turn);
  const drawing = typeof source === 'object' ? source : null;
  if (drawing?.choose) chooser(target, drawing);
  /** @type {string} */
  const text = typeof source === 'object' ? '' : typeof source === 'function' ? source(colours()) : source;
  const flow = typeof source === 'string' && FLOWCHART.test(text);
  const expanded = EXPANDED.has(target);
  const key = drawing ? `drawing\n${expanded ? 'expanded' : 'block'}\n${drawing.key}` : flow ? `flow\n${text}` : `${scheme()}\n${text}`;
  if (drawn.has(key)) {
    show(target, drawn.get(key));
    return;
  }
  const id = `diagram-${(counter += 1)}`;
  try {
    const svg = drawing ? await drawing.draw(id, expanded) : flow ? await drawFlowchart(await mermaid(), text, id) : (await (await mermaid()).render(id, text)).svg;
    drawn.set(key, svg);
    if (drawn.size > KEPT) drawn.delete(drawn.keys().next().value);
    if (turns.get(target) !== turn) return;
    show(target, svg);
  } catch (error) {
    document.getElementById(id)?.remove();
    document.getElementById(`d${id}`)?.remove();
    if (turns.get(target) !== turn) return;
    const pre = document.createElement('pre');
    pre.textContent = text;
    const line = document.createElement('p');
    line.className = 'diagram-error';
    line.textContent = 'This diagram could not be drawn.';
    target.replaceChildren(line, ...(text ? [pre] : []));
    target.classList.add('is-failed');
  }
}

/** The targets whose nodes tell their drawing's `choose`, and the drawing each tells now. */
const choosers = new WeakMap();

/**
 * Lets the person choose a node of the drawing in `target`, by the pointer or by Enter or Space,
 * unless it is drawn expanded.
 * @param {HTMLElement} target
 * @param {Drawing} drawing
 */
function chooser(target, drawing) {
  if (EXPANDED.has(target)) return;
  const bound = choosers.has(target);
  choosers.set(target, drawing);
  if (bound) return;
  /** @param {Event} event */
  const keyOf = (event) => /** @type {Element | null} */ (event.target)?.closest?.('[data-key][role="button"]')?.getAttribute('data-key') ?? null;
  target.addEventListener('click', (event) => {
    const key = keyOf(event);
    if (key) choosers.get(target)?.choose?.(key);
  });
  target.addEventListener('keydown', (event) => {
    const key = keyOf(event);
    if (!key || (event.key !== 'Enter' && event.key !== ' ')) return;
    event.preventDefault();
    choosers.get(target)?.choose?.(key);
  });
}

/**
 * Gives the diagram drawn in `target` its button that expands it, unless it is itself drawn
 * expanded.
 * @param {HTMLElement} target
 */
function expandable(target) {
  if (EXPANDED.has(target)) return;
  // The icon is a glyph, not an SVG, so that the diagram's SVG stays the only one it holds.
  const button = h(
    'button',
    { type: 'button', class: 'diagram-expand', 'aria-label': 'Expand the diagram', title: 'Expand the diagram' },
    h('span', { 'aria-hidden': 'true' }, '⤢'),
  );
  button.addEventListener('click', () => expand(target, button));
  target.append(button);
}

/**
 * Where the button that expands a diagram lies: its block's key and its place among the block's
 * buttons, by which it is found again once the page has drawn the block afresh.
 * @param {HTMLElement} button
 */
function placeOf(button) {
  const block = /** @type {HTMLElement | null} */ (button.closest('article.block'));
  const index = block ? [...block.querySelectorAll('.diagram-expand')].indexOf(button) : -1;
  return { key: block?.dataset.key ?? null, index };
}

/**
 * The button at `place`, as the page now draws it.
 * @param {{ key: string | null, index: number }} place
 * @returns {HTMLElement | null}
 */
function buttonAt(place) {
  if (!place.key || place.index < 0) return null;
  const block = [...document.querySelectorAll('article.block')].find((each) => /** @type {HTMLElement} */ (each).dataset.key === place.key);
  return /** @type {HTMLElement | null} */ (block?.querySelectorAll('.diagram-expand')[place.index] ?? null);
}

/**
 * Opens the diagram drawn in `target` in the page's modal, named by its block's heading, drawn
 * from the same source and fitted to the modal.
 * @param {HTMLElement} target
 * @param {HTMLElement} button
 */
export function expand(target, button) {
  const source = shown.get(target);
  if (source === undefined) return;
  const heading = target.closest('article.block')?.querySelector('h4')?.textContent?.trim();
  const place = placeOf(button);
  const content = h('div', { class: 'zoom-content' });
  EXPANDED.add(content);
  const zoom = zoomStage(content);
  // A browser that gives a clicked button no focus would give it back to nothing.
  button.focus();
  modal.show('diagram', {
    title: heading || 'Diagram',
    content: zoom.element,
    class: 'modal--diagram',
    refocus: () => buttonAt(place),
  });
  zoom.stage.focus();
  // The modal has its size once it is open; the diagram, once it is drawn.
  void draw(content, source).then(() => requestAnimationFrame(zoom.fit));
}

/** Draws again each diagram still on the page, in the scheme the browser now asks for. */
function redrawAll() {
  for (const [target, source] of shown) {
    if (target.isConnected) void draw(target, source);
    else shown.delete(target);
  }
}

globalThis.matchMedia?.('(prefers-color-scheme: dark)').addEventListener?.('change', redrawAll);

/**
 * Draws each code block marked `mermaid` under `root` as a diagram in its place.
 * @param {ParentNode} root
 */
export function drawAll(root) {
  // The diagrams of a page drawn before this one are no longer on it, and are let go.
  for (const target of shown.keys()) {
    if (!target.isConnected) shown.delete(target);
  }
  for (const code of root.querySelectorAll('pre > code.language-mermaid')) {
    const pre = code.parentElement;
    if (!pre || pre.dataset.drawn) continue;
    pre.dataset.drawn = 'true';
    const target = document.createElement('div');
    target.className = 'diagram';
    pre.replaceWith(target);
    void draw(target, code.textContent ?? '');
  }
}
