// Building the page's elements. Every text a block holds goes in as text, never as markup; the
// only markup the page inserts is the HTML `loft` rendered from Markdown, through `html()`.

import { parse } from './route.js';

/**
 * @typedef {string | number | Node | null | undefined | false} Leaf
 * @typedef {Leaf | Leaf[] | Leaf[][] | Leaf[][][]} Child
 * @typedef {Record<string, any>} Attributes
 */

/**
 * An element: `tag` with `attributes` and `children`. A `class` may be a list, an `on…`
 * attribute a listener, and `dataset` an object; `null`, `undefined` and `false` children are
 * left out.
 * @param {string} tag
 * @param {Attributes} [attributes]
 * @param {...(Child | Child[])} children
 * @returns {HTMLElement}
 */
export function h(tag, attributes = {}, ...children) {
  const element = document.createElement(tag);
  for (const [name, value] of Object.entries(attributes)) {
    if (value === null || value === undefined || value === false) continue;
    if (name === 'class') {
      element.className = Array.isArray(value) ? value.filter(Boolean).join(' ') : String(value);
    } else if (name === 'dataset') {
      for (const [key, data] of Object.entries(value)) element.dataset[key] = String(data);
    } else if (name.startsWith('on') && typeof value === 'function') {
      element.addEventListener(name.slice(2), value);
    } else if (value === true) {
      element.setAttribute(name, '');
    } else {
      element.setAttribute(name, String(value));
    }
  }
  append(element, children);
  return element;
}

/**
 * An SVG element.
 * @param {string} tag
 * @param {Attributes} [attributes]
 * @param {...(Child | Child[])} children
 * @returns {SVGElement}
 */
export function svg(tag, attributes = {}, ...children) {
  const element = document.createElementNS('http://www.w3.org/2000/svg', tag);
  for (const [name, value] of Object.entries(attributes)) {
    if (value === null || value === undefined || value === false) continue;
    if (name.startsWith('on') && typeof value === 'function') {
      element.addEventListener(name.slice(2), value);
    } else {
      element.setAttribute(name, String(value));
    }
  }
  append(element, children);
  return element;
}

/**
 * @param {Element} element
 * @param {(Child | Child[])[]} children
 */
function append(element, children) {
  for (const child of children.flat(Infinity)) {
    if (child === null || child === undefined || child === false) continue;
    element.append(child instanceof Node ? child : document.createTextNode(String(child)));
  }
}

/** A color as a code span may hold it: `#` and three, four, six or eight hexadecimal digits. */
const COLOR = /^#(?:[0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i;

/**
 * The ink that reads more clearly on the color `hex`, its alpha left aside: black on a light
 * color and white on a dark one, each at least 4.5 to 1 (the page's own dark ink, a shade off
 * black, would fall short on a mid gray).
 * @param {string} hex
 */
export function inkOn(hex) {
  const digits = hex.slice(1);
  const full = digits.length <= 4 ? [...digits.slice(0, 3)].map((digit) => digit + digit).join('') : digits.slice(0, 6);
  const n = Number.parseInt(full, 16);
  const luminance = [n >> 16, (n >> 8) & 255, n & 255]
    .map((c) => c / 255)
    .map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4))
    .reduce((sum, c, i) => sum + c * [0.2126, 0.7152, 0.0722][i], 0);
  return luminance > 0.179 ? '#000000' : '#ffffff';
}

/**
 * Draws each code span under `root` that holds only a color on that color; a code block's
 * code is left as it is.
 * @param {ParentNode} root
 */
function swatch(root) {
  for (const code of root.querySelectorAll('code')) {
    const text = code.textContent?.trim() ?? '';
    if (!COLOR.test(text) || code.closest('pre')) continue;
    const span = /** @type {HTMLElement} */ (code);
    span.classList.add('color');
    span.style.backgroundColor = text;
    span.style.color = inkOn(text);
  }
}

/**
 * A container holding HTML that `loft` rendered from a block's Markdown, with raw HTML already
 * turned to text and unsafe links undone, each table in a wrapper that scrolls sideways where the
 * table is wider than the block, and its links and colors dressed as `dress()` dresses them.
 * @param {string} rendered
 * @param {string} [className]
 * @returns {HTMLElement}
 */
export function html(rendered, className = 'prose') {
  const element = document.createElement('div');
  element.className = className;
  element.innerHTML = rendered ?? '';
  dress(element);
  for (const table of element.querySelectorAll('table')) {
    const wrap = document.createElement('div');
    wrap.className = 'table-wrap';
    table.replaceWith(wrap);
    wrap.append(table);
  }
  return element;
}

/**
 * A field of a block's Markdown, such as a task's detail: the HTML `loft` rendered from it, as
 * `html()` holds it, or its text where the page server gave no HTML; nothing where it is empty.
 * @param {string | undefined} rendered
 * @param {string | undefined} text
 * @param {string} className
 * @returns {HTMLElement | null}
 */
export function markdown(rendered, text, className) {
  if (rendered) return html(rendered, `${className} prose`);
  return text ? h('p', { class: className }, text) : null;
}

/**
 * One line of HTML that `loft` rendered from Markdown, such as a node's name, its links and
 * colors dressed as `dress()` dresses them.
 * @param {string} rendered
 */
export function inline(rendered) {
  const element = document.createElement('span');
  element.className = 'inline';
  element.innerHTML = rendered;
  dress(element);
  return element;
}

/**
 * Dresses HTML that `loft` rendered from Markdown, in a block and in a name alike, so the two
 * cannot drift apart: a web link opens a new tab, a link within the page keeps the Yard Log
 * shown, and a color in a code span is drawn on that color. The empty anchor comrak writes in
 * each heading goes: unseen, it would only be a stop of the keyboard that names its heading again.
 * @param {Element} element
 */
function dress(element) {
  for (const anchor of element.querySelectorAll('a.anchor:empty')) anchor.remove();
  newTabs(element);
  stayOnPage(element);
  swatch(element);
}

/**
 * Opens each link of rendered Markdown to a web address in a new tab, so that following one
 * leaves the Yard page as it was, and gives the page it opens no hold on this one. A link to this
 * page at a Yard Log becomes that Yard Log's route, which the page shows in its own tab; a
 * `mailto:` link and a link within the page stay as they are.
 * @param {Element} element
 */
function newTabs(element) {
  for (const link of element.querySelectorAll('a[href]')) {
    // The address as written: a link within the page would resolve to this page's own `http:`.
    const href = link.getAttribute('href') ?? '';
    if (/^https?:\/\//i.test(href)) {
      const url = new URL(href);
      if (url.origin === location.origin && url.pathname === location.pathname && parse(url.hash)) {
        link.setAttribute('href', url.hash);
        continue;
      }
      link.setAttribute('target', '_blank');
      link.setAttribute('rel', 'noopener noreferrer');
      // Read out as the link's description, so no one is moved off the page unwarned, while its
      // words and the page's look stay as they were.
      link.setAttribute('aria-describedby', 'new-tab-note');
    }
  }
}

/**
 * Keeps the Yard Log shown when a link of rendered Markdown to a place within the page, such as
 * a footnote's, is followed. The page's address holds its route, so the link brings its place
 * into view and moves the focus there instead of changing the address, and before it is opened
 * elsewhere, in a new tab or from the browser's menu, it takes the address of the Yard Log shown.
 * Each block numbers its footnotes from 1, so the place is looked for within the link's own block
 * alone. A link to the address of a Yard Log is a route, which the browser follows as written.
 * @param {Element} element
 */
function stayOnPage(element) {
  for (const link of element.querySelectorAll('a[href^="#"]')) {
    const href = link.getAttribute('href') ?? '';
    if (parse(href)) continue;
    const place = placeOf(element, href.slice(1));
    // A pointer is down before any menu opens, and the focus is in before the keyboard's menu key.
    const shown = () => link.setAttribute('href', location.hash || '#');
    for (const type of ['pointerdown', 'focus']) link.addEventListener(type, shown);
    link.addEventListener('click', (event) => {
      // A click that asks for another tab or window is the browser's, to the address `shown` set.
      const click = /** @type {MouseEvent} */ (event);
      if (click.button || click.metaKey || click.ctrlKey || click.shiftKey || click.altKey) return;
      event.preventDefault();
      if (!place) return;
      // Focusable, so the keyboard and a screen reader go on from the place, as they would after
      // a jump the browser made.
      if (place.tabIndex < 0) place.setAttribute('tabindex', '-1');
      place.scrollIntoView({ block: 'center' });
      place.focus({ preventScroll: true });
    });
  }
}

/**
 * The element of `element` that `name`, an address within the page without its `#`, names. The
 * id is matched as written, not decoded: `loft` writes an id and the address that names it alike,
 * encoding a letter outside ASCII in both. A heading's id holds GitHub's prefix, which a link to
 * it leaves out.
 * @param {Element} element
 * @param {string} name
 * @returns {HTMLElement | undefined}
 */
function placeOf(element, name) {
  // The prefix `header_id_prefix` gives in `markdown.rs`.
  const heading = `user-content-${name}`;
  return /** @type {HTMLElement[]} */ ([...element.querySelectorAll('[id]')]).find((node) => node.id === name || node.id === heading);
}

/**
 * Adds `children` at the end of `element`, as `h` adds them.
 * @param {Element} element
 * @param {...(Child | Child[])} children
 */
export function add(element, ...children) {
  append(element, children);
}

/**
 * Replaces the children of `element`.
 * @param {Element} element
 * @param {...(Child | Child[])} children
 */
export function replace(element, ...children) {
  element.replaceChildren();
  append(element, children);
}
