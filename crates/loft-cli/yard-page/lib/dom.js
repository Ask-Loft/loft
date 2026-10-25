// Building the page's elements. Every text a block holds goes in as text, never as markup; the
// only markup the page inserts is the HTML `loft` rendered from Markdown, through `html()`.

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

/** A colour as a code span may hold it: `#` and three, four, six or eight hexadecimal digits. */
const COLOUR = /^#(?:[0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i;

/**
 * The ink that reads more clearly on the colour `hex`, its alpha left aside: black on a light
 * colour and white on a dark one, each at least 4.5 to 1 (the page's own dark ink, a shade off
 * black, would fall short on a mid grey).
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
 * Draws each code span under `root` that holds only a colour on that colour; a code block's
 * code is left as it is.
 * @param {ParentNode} root
 */
function swatch(root) {
  for (const code of root.querySelectorAll('code')) {
    const text = code.textContent?.trim() ?? '';
    if (!COLOUR.test(text) || code.closest('pre')) continue;
    const span = /** @type {HTMLElement} */ (code);
    span.classList.add('colour');
    span.style.backgroundColor = text;
    span.style.color = inkOn(text);
  }
}

/**
 * A container holding HTML that `loft` rendered from a block's Markdown, with raw HTML already
 * turned to text and unsafe links undone, each table in a wrapper that scrolls sideways where the
 * table is wider than the block, and each colour in a code span drawn on that colour.
 * @param {string} rendered
 * @param {string} [className]
 * @returns {HTMLElement}
 */
export function html(rendered, className = 'prose') {
  const element = document.createElement('div');
  element.className = className;
  element.innerHTML = rendered ?? '';
  for (const table of element.querySelectorAll('table')) {
    const wrap = document.createElement('div');
    wrap.className = 'table-wrap';
    table.replaceWith(wrap);
    wrap.append(table);
  }
  swatch(element);
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
 * One line of HTML that `loft` rendered from Markdown, such as a node's name, each colour in a
 * code span drawn on that colour.
 * @param {string} rendered
 */
export function inline(rendered) {
  const element = document.createElement('span');
  element.className = 'inline';
  element.innerHTML = rendered;
  swatch(element);
  return element;
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
