// The page server, as the page reaches it: every request goes to the server that served the
// page, which takes the page's own `Origin` for what changes something.

/**
 * @typedef {{ status: number, body: any }} Reply
 */

/**
 * @param {string} path
 * @returns {Promise<Reply>}
 */
export async function get(path) {
  const response = await fetch(path, { headers: { Accept: 'application/json' } });
  return { status: response.status, body: await body(response) };
}

/**
 * @param {string} path
 * @param {unknown} sent
 * @returns {Promise<Reply>}
 */
export async function post(path, sent) {
  const response = await fetch(path, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
    body: JSON.stringify(sent),
  });
  return { status: response.status, body: await body(response) };
}

/** @param {Response} response */
async function body(response) {
  const text = await response.text();
  try {
    return JSON.parse(text);
  } catch {
    return { errors: [{ kind: 'response', message: text || response.statusText }] };
  }
}

/** Every tree served and its branches' Yard Logs, and what waits on the person. */
export function trees() {
  return get('/api/trees');
}

/**
 * A branch's Yard Log in a tree.
 * @param {string} tree
 * @param {string} branch
 */
export function log(tree, branch) {
  const query = new URLSearchParams({ tree, branch });
  return get(`/api/log?${query}`);
}

/**
 * The person's answers and decisions for a branch's Yard Log.
 * @param {string} tree
 * @param {string} branch
 * @param {object[]} answers
 */
export function answer(tree, branch, answers) {
  return post('/api/answer', { tree, branch, answers });
}

/**
 * Listens to the page server's stream: `onHello` with its protocol each time it connects, and
 * `onChange` each time a Yard Log it serves changes. Gives a function that stops listening.
 * @param {{ onHello: (protocol: any) => void, onChange: () => void,
 *   onState?: (open: boolean) => void }} handlers
 */
export function follow({ onHello, onChange, onState }) {
  const source = new EventSource('/api/stream');
  source.addEventListener('hello', (event) => {
    onState?.(true);
    try {
      onHello(JSON.parse(/** @type {MessageEvent} */ (event).data));
    } catch {
      onHello({});
    }
  });
  source.addEventListener('change', () => onChange());
  source.addEventListener('error', () => onState?.(false));
  return () => source.close();
}
