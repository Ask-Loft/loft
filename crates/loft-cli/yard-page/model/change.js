// What a change the page server streams asks the page to load again.

/**
 * What a change touched: everything, where `all`; the trees served, where `trees`; and each
 * branch's Yard Log in a tree, with `log` where its own files changed and not only its sessions'.
 * @typedef {{ all?: boolean, trees?: boolean,
 *   logs?: { tree: string, branch: string, log: boolean }[] }} Change
 */

/**
 * What `change` asks the page to load again: the listing where the trees or a Yard Log's own
 * files changed; the Yard Log shown at `route` where its files or its sessions' changed.
 * @param {Change} change
 * @param {{ tree: string, branch: string } | null} route
 * @returns {{ listing: boolean, log: boolean }}
 */
export function wantedBy(change, route) {
  if (change.all || change.trees) return { listing: true, log: true };
  const logs = change.logs ?? [];
  return {
    listing: logs.some((log) => log.log),
    log: Boolean(route) && logs.some((log) => log.tree === route?.tree && log.branch === route?.branch),
  };
}
