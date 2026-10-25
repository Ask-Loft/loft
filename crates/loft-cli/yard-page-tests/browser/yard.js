// What the browser suite does besides the browser: it writes to Yard Log as an agent does and
// answers in the terminal as the person does, each through the same `loft` the Rust test built
// and in front of the same git simulator, and it reaches the page server as the mod does.

import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const file = process.env.YARD_BROWSER_SETUP;
if (!file) throw new Error('YARD_BROWSER_SETUP names no file: run the suite through tests/yard_page_browser.rs');

/** @type {{ url: string, token: string, tree: string, branch: string, driver: Driver, other: Driver }} */
export const setup = JSON.parse(readFileSync(file, 'utf8'));

/** @typedef {{ program: string, cwd: string, env: Record<string, string | null> }} Driver */

const scratch = mkdtempSync(join(tmpdir(), 'yard-browser-'));
let inputs = 0;

/**
 * Runs `loft yard <args> --format json` as `driver` gives it, with `input` as its `--input`.
 * @param {Driver} driver
 * @param {string[]} args
 * @param {unknown} [input]
 */
function yard(driver, args, input) {
  const env = { ...process.env };
  for (const [name, value] of Object.entries(driver.env)) {
    if (value === null) delete env[name];
    else env[name] = value;
  }
  const all = ['yard', ...args];
  if (input !== undefined) {
    inputs += 1;
    const path = join(scratch, `input-${inputs}.json`);
    writeFileSync(path, JSON.stringify(input));
    all.push('--input', path);
  }
  all.push('--format', 'json');
  const run = spawnSync(driver.program, all, { cwd: driver.cwd, env, encoding: 'utf8' });
  if (run.status !== 0) throw new Error(`loft ${all.join(' ')} exited ${run.status}: ${run.stdout}${run.stderr}`);
  return JSON.parse(run.stdout);
}

/**
 * Writes `batch` as session s1's main agent of the tree the page opens on.
 * @param {object[]} batch
 * @param {string[]} [extra]
 */
export function write(batch, extra = []) {
  return yard(setup.driver, ['write', '--session', 's1', ...extra], batch);
}

/**
 * Writes `batch` in the other tree the page server serves.
 * @param {object[]} batch
 */
export function writeOther(batch) {
  return yard(setup.other, ['write', '--session', 's9'], batch);
}

/**
 * Withdraws the submission `key` as session s1's main agent.
 * @param {string} key
 */
export function withdraw(key) {
  return yard(setup.driver, ['withdraw', '--session', 's1', '--submission', key]);
}

/**
 * Answers and decides in the terminal, as Claude Code records them.
 * @param {object[]} answers
 */
export function answerInTerminal(answers) {
  return yard(setup.driver, ['answer', '--session', 's1'], answers);
}

/** The Yard Log the page opens on, as the page server serves it. */
export async function read() {
  const query = new URLSearchParams({ tree: setup.tree, branch: setup.branch });
  const response = await fetch(new URL(`/api/log?${query}`, setup.url));
  return response.json();
}

/**
 * `POST path` with `body` and the control token, as the mod sends it.
 * @param {string} path
 * @param {unknown} body
 */
export async function postAsMod(path, body) {
  const response = await fetch(new URL(path, setup.url), {
    method: 'POST',
    headers: { Authorization: `Bearer ${setup.token}`, 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  return { status: response.status, body: await response.json() };
}

/** The address of the Yard Log the page opens on, at its block `block` if one is given. */
export function address(block) {
  const query = new URLSearchParams({ tree: setup.tree, branch: setup.branch });
  if (block) query.set('block', block);
  return `${setup.url}#${query}`;
}

/** The ISO time `seconds` from now, as Yard Log writes times. */
export function since(seconds) {
  return new Date(Date.now() + seconds * 1000).toISOString().replace(/\.\d+Z$/, 'Z');
}
