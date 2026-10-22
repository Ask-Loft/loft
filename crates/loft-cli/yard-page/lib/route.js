// The page's address: a branch's Yard Log is opened at `#tree=<dir>&branch=<name>`, the
// address `loft` gives it, so that each has an address of its own.

/**
 * @typedef {{ tree: string, branch: string, block?: string }} Route
 */

/**
 * The Yard Log a hash names, or null for the list of Yard Logs.
 * @param {string} hash
 * @returns {Route | null}
 */
export function parse(hash) {
  const params = new URLSearchParams(String(hash ?? '').replace(/^#/, ''));
  const tree = params.get('tree');
  const branch = params.get('branch');
  if (!tree || !branch) return null;
  const block = params.get('block') ?? undefined;
  return { tree, branch, block };
}

/**
 * The hash of a branch's Yard Log, and of a block in it.
 * @param {Route} route
 */
export function format({ tree, branch, block }) {
  const params = new URLSearchParams({ tree, branch });
  if (block) params.set('block', block);
  return `#${params}`;
}
