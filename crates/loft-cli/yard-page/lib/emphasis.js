// The hover layer of a diagram the page draws itself: pointing at a node, or focusing it, brings
// forward the node, its edges and their labels and the nodes at their other ends, and fades the
// rest of the diagram until the pointer or the focus leaves it. Panels never fade.

/** What takes part: the nodes that bring their neighbours forward, and what fades. */
const NODES = '.flow-node, .spec-card';

/**
 * Brings forward the node `id` of `drawn` and what it joins.
 * @param {Element} drawn the diagram's SVG
 * @param {string} id
 */
export function bringForward(drawn, id) {
  const near = new Set([id]);
  for (const edge of drawn.querySelectorAll('[data-from]')) {
    const [from, to] = [edge.getAttribute('data-from'), edge.getAttribute('data-to')];
    const joined = from === id || to === id;
    edge.classList.toggle('is-near', joined);
    if (joined) {
      near.add(from ?? '');
      near.add(to ?? '');
    }
  }
  for (const node of drawn.querySelectorAll(NODES)) node.classList.toggle('is-near', near.has(node.getAttribute('data-id') ?? ''));
  drawn.classList.add('is-emphasis');
}

/**
 * Lets every node of `drawn` fall back.
 * @param {Element} drawn
 */
export function fallBack(drawn) {
  drawn.classList.remove('is-emphasis');
  for (const each of drawn.querySelectorAll('.is-near')) each.classList.remove('is-near');
}

/**
 * Gives the diagrams drawn into `target` their hover layer, once, whatever is drawn into it later.
 * @param {HTMLElement} target
 */
export function emphasis(target) {
  if (target.dataset.emphasis) return;
  target.dataset.emphasis = 'on';
  /** @param {Event} event */
  const nodeOf = (event) => /** @type {Element | null} */ (event.target)?.closest?.(NODES) ?? null;
  /** @param {Event} event */
  const enter = (event) => {
    const node = nodeOf(event);
    const drawn = node?.closest('svg.flow');
    if (node && drawn) bringForward(drawn, node.getAttribute('data-id') ?? '');
  };
  /** @param {Event} event */
  const leave = (event) => {
    const node = nodeOf(event);
    const to = /** @type {any} */ (event).relatedTarget;
    if (!node || (to && node.contains(to))) return;
    const drawn = node.closest('svg.flow');
    if (drawn) fallBack(drawn);
  };
  target.addEventListener('pointerover', enter);
  target.addEventListener('focusin', enter);
  target.addEventListener('pointerout', leave);
  target.addEventListener('focusout', leave);
}
