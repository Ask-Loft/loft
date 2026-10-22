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

/**
 * A container holding HTML that `loft` rendered from a block's Markdown, with raw HTML already
 * turned to text and unsafe links undone.
 * @param {string} rendered
 * @param {string} [className]
 * @returns {HTMLElement}
 */
export function html(rendered, className = 'prose') {
  const element = document.createElement('div');
  element.className = className;
  element.innerHTML = rendered ?? '';
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
