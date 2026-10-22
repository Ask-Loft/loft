// A DOM emulator in place of the browser, so that the page's modules run under `node --test`:
// imported first by each test file, before any module of the page.

import { Window } from 'happy-dom';

const window = new Window({ url: 'http://127.0.0.1:7411/' });

for (const name of [
  'window',
  'document',
  'Node',
  'Element',
  'HTMLElement',
  'HTMLDetailsElement',
  'HTMLInputElement',
  'HTMLTextAreaElement',
  'SVGElement',
  'Event',
  'MessageEvent',
  'localStorage',
  'location',
]) {
  Object.defineProperty(globalThis, name, { value: name === 'window' ? window : window[name], configurable: true, writable: true });
}

// A test never compares a node with `assert.equal`: a failed one prints both values in full, and
// a node reaches the whole emulated browser, which takes minutes to print. It asserts
// `assert.ok(!node)` that a node is absent, which prints nothing of it.

/** Clears what the page kept in this browser. */
export function forget() {
  window.localStorage.clear();
}

/**
 * A fresh element to draw into.
 * @returns {HTMLElement}
 */
export function root() {
  const element = window.document.createElement('div');
  window.document.body.replaceChildren(element);
  return /** @type {any} */ (element);
}
