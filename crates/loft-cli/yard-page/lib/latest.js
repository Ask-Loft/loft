// Reads that may overlap, of which only the one begun last is applied: a read the page began for
// an address it has since left, or overtaken by a later read, answers nothing.

/**
 * A reader whose reads resolve to their value only while no later read has begun, and to `null`
 * once one has.
 * @returns {<T>(read: Promise<T>) => Promise<T | null>}
 */
export function latest() {
  let begun = 0;
  return async (read) => {
    const ticket = ++begun;
    const value = await read;
    return ticket === begun ? value : null;
  };
}
