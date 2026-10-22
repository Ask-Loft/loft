// Diagrams: each code block of a block's Markdown marked `mermaid`, and each diagram the page
// builds, drawn by the copy of Mermaid the page server serves with the page. Mermaid is loaded
// the first time a diagram is drawn, in its strict mode, which draws no HTML the source holds.

let loading = null;

/** @returns {Promise<any>} */
function mermaid() {
  if (loading) return loading;
  loading = new Promise((resolve, reject) => {
    const found = /** @type {any} */ (globalThis).mermaid;
    if (found) {
      resolve(found);
      return;
    }
    const script = document.createElement('script');
    script.src = 'vendor/mermaid.min.js';
    script.onload = () => {
      const loaded = /** @type {any} */ (globalThis).mermaid;
      const dark = globalThis.matchMedia?.('(prefers-color-scheme: dark)').matches;
      loaded.initialize({ startOnLoad: false, securityLevel: 'strict', theme: dark ? 'dark' : 'neutral' });
      resolve(loaded);
    };
    script.onerror = () => reject(new Error('the diagram library did not load'));
    document.head.append(script);
  });
  return loading;
}

let counter = 0;

/** Each source drawn, with what it was drawn as, so that a page drawn again draws at once. */
const drawn = new Map();

/**
 * Draws `source` into `target`; where it does not parse, shows the source with a line saying
 * it could not be drawn.
 * @param {HTMLElement} target
 * @param {string} source
 */
export async function draw(target, source) {
  if (drawn.has(source)) {
    target.innerHTML = drawn.get(source);
    target.classList.add('is-drawn');
    return;
  }
  const id = `diagram-${(counter += 1)}`;
  try {
    const library = await mermaid();
    const { svg } = await library.render(id, source);
    drawn.set(source, svg);
    target.innerHTML = svg;
    target.classList.add('is-drawn');
  } catch (error) {
    document.getElementById(id)?.remove();
    document.getElementById(`d${id}`)?.remove();
    const pre = document.createElement('pre');
    pre.textContent = source;
    const line = document.createElement('p');
    line.className = 'diagram-error';
    line.textContent = 'This diagram could not be drawn.';
    target.replaceChildren(line, pre);
    target.classList.add('is-failed');
  }
}

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
