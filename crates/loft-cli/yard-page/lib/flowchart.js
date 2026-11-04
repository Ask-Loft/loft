// A Mermaid flowchart drawn by the page itself: Mermaid parses it, the page measures each label in
// its own face, ELK's layered layout places it, and the page draws it (`views/flowchart-svg.js`).
// ELK is loaded the first time a flowchart is drawn, from the copy the page server serves.

import { svg } from './dom.js';
import { PAD, graphOf, placed } from '../model/flowchart.js';
import { flowchartSvg } from '../views/flowchart-svg.js';

/** Whether `text` is a flowchart, which the page draws itself. */
export const FLOWCHART = /^\s*(?:%%[^\n]*\n\s*)*(?:flowchart|graph)\b/;

let loading = null;

/** @returns {Promise<any>} ELK's layout engine. */
function elk() {
  if (!loading) {
    loading = new Promise((resolve, reject) => {
      const found = /** @type {any} */ (globalThis).ELK;
      if (found) {
        resolve(new found());
        return;
      }
      const script = document.createElement('script');
      script.src = 'vendor/elk.bundled.js';
      script.onload = () => resolve(new (/** @type {any} */ (globalThis).ELK)());
      script.onerror = () => reject(new Error('the layout library did not load'));
      document.head.append(script);
    });
  }
  return loading;
}

/**
 * Measures labels as the page's stylesheet sets a flowchart's text: in an SVG of the class
 * `flow` laid off the page.
 * @returns {{ measure: (lines: string[]) => { width: number, height: number }, done: () => void }}
 */
function ruler() {
  const text = svg('text', { class: 'flow-label' });
  const root = svg('svg', { class: 'flow', width: 0, height: 0, 'aria-hidden': 'true', style: 'position:absolute;left:-9999px;top:0;visibility:hidden' }, text);
  document.body.append(root);
  return {
    measure: (lines) => {
      let width = 0;
      for (const line of lines) {
        text.textContent = line || ' ';
        width = Math.max(width, /** @type {SVGTextElement} */ (text).getComputedTextLength?.() ?? line.length * 7.5);
      }
      return { width: Math.ceil(width), height: Math.max(1, lines.length) * PAD.line };
    },
    done: () => root.remove(),
  };
}

/**
 * The SVG of the flowchart `text`, drawn by the page.
 * @param {any} library Mermaid, set up
 * @param {string} text
 * @param {string} id unique on the page
 * @returns {Promise<string>} the SVG's markup
 * @throws where Mermaid cannot parse the flowchart
 */
export async function drawFlowchart(library, text, id) {
  const diagram = await library.mermaidAPI.getDiagramFromText(text);
  const db = diagram.db;
  const data = db.getData();
  const kept = db.getClasses?.() ?? {};
  /** @type {Record<string, string[]>} */
  const classes = Object.fromEntries([...(kept instanceof Map ? kept.entries() : Object.entries(kept))].map(([name, each]) => [name, each?.styles ?? []]));
  await document.fonts?.ready;
  const { measure, done } = ruler();
  let graph;
  try {
    graph = graphOf(data, classes, db.getDirection?.() ?? 'TB', measure);
  } finally {
    done();
  }
  const laid = await (await elk()).layout(graph);
  return flowchartSvg(placed(laid), id).outerHTML;
}
