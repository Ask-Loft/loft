// Diagrams: each code block of a block's Markdown marked `mermaid`, and each diagram the page
// builds, drawn by the copy of Mermaid the page server serves with the page. Mermaid is loaded
// the first time a diagram is drawn, in its strict mode, which draws no HTML the source holds.
// It draws in the page's own colours for the scheme the browser asks for, and each diagram on
// the page is drawn again when the browser changes its scheme.

/** The colours of the page's light scheme, for a page whose stylesheet sets none. */
const LIGHT = {
  font: '"Inter", ui-sans-serif, system-ui, sans-serif',
  surface: '#ffffff',
  surface2: '#f3f7fa',
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

/** @param {Colours} c */
function theme(c) {
  return {
    darkMode: scheme() === 'dark',
    fontFamily: c.font,
    background: c.surface,
    primaryColor: c.surface2,
    primaryTextColor: c.ink,
    primaryBorderColor: c.ink,
    secondaryColor: c.mist,
    secondaryTextColor: c.ink,
    secondaryBorderColor: c.rail,
    tertiaryColor: c.surface,
    tertiaryTextColor: c.ink,
    tertiaryBorderColor: c.line,
    mainBkg: c.surface2,
    nodeBorder: c.ink,
    lineColor: c.muted,
    textColor: c.ink,
    titleColor: c.ink,
    clusterBkg: c.mist,
    clusterBorder: c.rail,
    edgeLabelBackground: c.surface,
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

/** Each diagram drawn, with its source, so that a change of scheme draws it again. */
const shown = new Map();

/** The latest drawing asked of each diagram, so that an earlier one finishing late is dropped. */
const turns = new WeakMap();

/**
 * Draws `source` into `target`, or the source that `source` makes from the colours of the
 * scheme; where it does not parse, shows the source with a line saying it could not be drawn.
 * @param {HTMLElement} target
 * @param {string | ((colours: Colours) => string)} source
 */
export async function draw(target, source) {
  shown.set(target, source);
  const turn = (turns.get(target) ?? 0) + 1;
  turns.set(target, turn);
  const text = typeof source === 'function' ? source(colours()) : source;
  const key = `${scheme()}\n${text}`;
  if (drawn.has(key)) {
    target.innerHTML = drawn.get(key);
    target.classList.add('is-drawn');
    return;
  }
  const id = `diagram-${(counter += 1)}`;
  try {
    const library = await mermaid();
    const { svg } = await library.render(id, text);
    drawn.set(key, svg);
    if (turns.get(target) !== turn) return;
    target.innerHTML = svg;
    target.classList.add('is-drawn');
  } catch (error) {
    document.getElementById(id)?.remove();
    document.getElementById(`d${id}`)?.remove();
    if (turns.get(target) !== turn) return;
    const pre = document.createElement('pre');
    pre.textContent = text;
    const line = document.createElement('p');
    line.className = 'diagram-error';
    line.textContent = 'This diagram could not be drawn.';
    target.replaceChildren(line, pre);
    target.classList.add('is-failed');
  }
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
