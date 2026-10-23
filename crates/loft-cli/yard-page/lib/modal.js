// The page's modals: one native dialog for each name, with its title and a button that closes it
// above content that scrolls within it. A modal lies over the page darkened behind it; a docked
// one is a panel along the bottom of the window that leaves the page above it live. Every view
// that opens one opens it here. The dialog lies outside what the page draws again at each change,
// so a change drawn into it keeps its scroll and the field that has focus.

import * as dock from './dock.js';
import { h, replace } from './dom.js';

/**
 * @typedef {import('./dom.js').Child} Child
 * @typedef {{ title: string, content: Child | Child[], onClose?: () => void, class?: string,
 *   refocus?: () => HTMLElement | null, docked?: boolean }} Shown
 * `class` is a class of the dialog besides `modal`; `refocus` finds what takes the focus back
 * when what held it before the modal opened has left the page; `docked` opens it as a panel.
 */

/** @type {Map<string, { dialog: HTMLDialogElement, opener: Element | null, onClose?: () => void,
 *   refocus?: () => HTMLElement | null }>} */
const modals = new Map();

/** @param {string} name */
function idOf(name) {
  return `modal-${name.replace(/[^a-z0-9-]/gi, '-')}`;
}

/**
 * The dialog of the modal `name`, made afresh where it is not in the document.
 * @param {string} name
 */
function dialogOf(name) {
  const known = modals.get(name);
  if (known?.dialog.isConnected) return known;
  const id = idOf(name);
  const dialog = /** @type {HTMLDialogElement} */ (
    h('dialog', { class: 'modal', id, dataset: { modal: name }, 'aria-labelledby': `${id}-title` })
  );
  // Escape closes the modal as its button does, through `close`, whatever the browser would do.
  dialog.addEventListener('cancel', (event) => {
    event.preventDefault();
    close(name);
  });
  // A panel is no modal, so the browser sends it no `cancel`: Escape pressed within it closes it.
  dialog.addEventListener('keydown', (event) => {
    if (event.key !== 'Escape' || event.defaultPrevented || !dialog.classList.contains('modal--docked')) return;
    event.preventDefault();
    close(name);
  });
  dialog.addEventListener('close', () => closed(name));
  document.body.append(dialog);
  const entry = { dialog, opener: null };
  modals.set(name, entry);
  return entry;
}

/**
 * Draws `shown` into the dialog of `name`.
 * @param {HTMLDialogElement} dialog
 * @param {string} name
 * @param {Shown} shown
 */
function draw(dialog, name, shown) {
  const id = idOf(name);
  replace(
    dialog,
    shown.docked ? dock.edge() : null,
    h(
      'header',
      { class: 'modal-head' },
      h('h2', { id: `${id}-title` }, shown.title),
      h('button', { type: 'button', class: 'modal-close', 'aria-label': 'Close', onclick: () => close(name) }, '×'),
    ),
    h('div', { class: 'modal-body' }, shown.content),
  );
}

/**
 * Opens the modal `name` with `shown`, or draws `shown` into it where it is open already.
 * @param {string} name
 * @param {Shown} shown
 */
export function show(name, shown) {
  const entry = dialogOf(name);
  entry.onClose = shown.onClose;
  entry.refocus = shown.refocus;
  entry.dialog.className = ['modal', shown.docked && 'modal--docked', shown.class].filter(Boolean).join(' ');
  if (entry.dialog.open) {
    update(name, shown);
    return entry.dialog;
  }
  entry.opener = document.activeElement;
  draw(entry.dialog, name, shown);
  if (shown.docked) {
    dock.fit(entry.dialog);
    entry.dialog.show();
  } else {
    entry.dialog.showModal();
  }
  return entry.dialog;
}

/**
 * Draws `shown` into the modal `name` where it is open, keeping its scroll and the field that has
 * focus; a closed modal is left as it is.
 * @param {string} name
 * @param {Shown} shown
 */
export function update(name, shown) {
  const entry = modals.get(name);
  if (!entry?.dialog.isConnected || !entry.dialog.open) return;
  const dialog = entry.dialog;
  const title = dialog.querySelector('.modal-head h2');
  const body = /** @type {HTMLElement | null} */ (dialog.querySelector('.modal-body'));
  if (!title || !body) {
    draw(dialog, name, shown);
    return;
  }
  const scrolled = body.scrollTop;
  const focused = /** @type {HTMLInputElement | null} */ (document.activeElement);
  const focus = focused?.id && body.contains(focused) ? { id: focused.id, start: focused.selectionStart, end: focused.selectionEnd } : null;
  // Only the title and the content are drawn again: the frame that scrolls stays, and so does the
  // close button with its focus.
  title.textContent = shown.title;
  replace(body, shown.content);
  body.scrollTop = scrolled;
  if (!focus) return;
  const again = /** @type {HTMLInputElement | null} */ (document.getElementById(focus.id));
  if (!again || !dialog.contains(again)) return;
  again.focus();
  if (typeof focus.start === 'number' && 'setSelectionRange' in again) {
    try {
      again.setSelectionRange(focus.start, focus.end ?? focus.start);
    } catch {
      // An input that holds no text selection.
    }
  }
}

/**
 * Closes the modal `name` where it is open.
 * @param {string} name
 */
export function close(name) {
  const entry = modals.get(name);
  if (entry?.dialog.open) entry.dialog.close();
}

/**
 * Whether the modal `name` is open.
 * @param {string} name
 */
export function isOpen(name) {
  const entry = modals.get(name);
  return Boolean(entry?.dialog.isConnected && entry.dialog.open);
}

/**
 * The body of the modal `name`, where it is open: where a view finds what it drew there.
 * @param {string} name
 * @returns {HTMLElement | null}
 */
export function body(name) {
  return isOpen(name) ? /** @type {HTMLElement | null} */ (modals.get(name)?.dialog.querySelector('.modal-body') ?? null) : null;
}

/**
 * Once the modal `name` closed: the focus goes back to what held it before it opened, or where
 * that has left the page to what its opener finds in its place, and whoever opened it hears of it.
 * @param {string} name
 */
function closed(name) {
  const entry = modals.get(name);
  if (!entry) return;
  const held = /** @type {HTMLElement | null} */ (entry.opener);
  entry.opener = null;
  const opener = held?.isConnected ? held : entry.refocus?.();
  if (opener?.isConnected && typeof opener.focus === 'function') opener.focus();
  entry.onClose?.();
}
