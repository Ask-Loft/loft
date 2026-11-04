// A `spec-change` view's diagram drawn by the page: its graph laid out as nested panels and cards
// (`model/spec-graph.js`), its text measured in the page's face, placed by ELK and drawn as the
// page's own SVG (`views/flowchart-svg.js`).

import { layoutEngine, ruler } from './flowchart.js';
import { placed } from '../model/flowchart.js';
import { specGraph, trunks } from '../model/spec-graph.js';
import { flowchartSvg } from '../views/flowchart-svg.js';

/**
 * The SVG of the diagram of `graph`, with the Feature Group `open` opened.
 * @param {any} graph a `spec-change` report's graph
 * @param {string | null} open
 * @param {string} id unique on the page
 * @returns {Promise<string>} the SVG's markup
 */
export async function drawSpecChange(graph, open, id) {
  await document.fonts?.ready;
  const { measure, done } = ruler();
  let laid;
  try {
    laid = specGraph(graph, open, measure);
  } finally {
    done();
  }
  const layout = trunks(placed(await (await layoutEngine()).layout(laid)));
  return flowchartSvg(layout, id, 'Diagram of the specification’s change').outerHTML;
}

/**
 * A short digest of `text`, by which a drawing is kept.
 * @param {string} text
 */
export function digest(text) {
  let hash = 0x811c9dc5;
  for (let index = 0; index < text.length; index += 1) hash = Math.imul(hash ^ text.charCodeAt(index), 0x01000193) >>> 0;
  return hash.toString(36);
}
