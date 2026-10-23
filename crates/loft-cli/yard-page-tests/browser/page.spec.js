// The Yard page in a headless Chromium, served by `loft yard serve`, while an agent writes to
// Yard Log and the person answers in the terminal through the same `loft`. The tests share one
// Yard Log and run in order: each adds what it needs to what the ones before it left.

import { expect, test } from '@playwright/test';

import { address, answerInTerminal, postAsMod, read, setup, since, withdraw, write, writeOther } from './yard.js';

/** Live changes reach an open page within this many milliseconds. */
const LIVE = 2000;

/** A question `id` sent to the person, under the stage `shape`. */
function ask(id, prompt) {
  write(
    [{ id, kind: 'question', stage: 'shape', prompt, header: id, options: [{ label: 'Red', recommended: true }, { label: 'Blue' }] }],
    ['--send'],
  );
}

/** An approval `id` sent to the person, under the stage `shape`; gives the submission's key. */
function approval(id, title) {
  return write([{ id, kind: 'approval', stage: 'shape', title, markdown: `May I ${title.toLowerCase()}?` }], ['--send']).submission;
}

/** Keeps the page from hearing of changes, so that it shows what it loaded until it loads again. */
async function deaf(page) {
  await page.route('**/api/stream', (route) => route.abort());
}

test('the person can fold and open each stage, the active one open and the planned one folded', async ({ page }) => {
  await page.goto(address());
  const shape = page.locator('#block-shape');
  const build = page.locator('#block-build');
  await expect(shape).toHaveAttribute('open', '');
  await expect(build).not.toHaveAttribute('open', '');
  await build.locator('summary').click();
  await expect(build).toHaveAttribute('open', '');
  await shape.locator('summary').click();
  await expect(shape).not.toHaveAttribute('open', '');
});

test('choosing a listed branch opens its Yard Log at its own address', async ({ page }) => {
  await page.goto(setup.url);
  await page.locator('.log-item a', { hasText: setup.branch }).first().click();
  await expect(page.locator('#log-title')).toBeVisible();
  expect(new URLSearchParams(new URL(page.url()).hash.slice(1)).get('tree')).toBe(setup.tree);
});

test('a diagram is drawn from the vendored Mermaid, and one that does not parse shows its source', async ({ page }) => {
  write([
    { id: 'drawn', kind: 'note', stage: 'shape', title: 'A diagram', markdown: '```mermaid\nflowchart LR\n  shelf --> reader\n```' },
    { id: 'broken', kind: 'note', stage: 'shape', title: 'A broken diagram', markdown: '```mermaid\nflowchart LR\n  shelf --> --> ((\n```' },
  ]);
  await page.goto(address());
  await expect(page.locator('#block-drawn svg')).toBeVisible({ timeout: 10_000 });
  const broken = page.locator('#block-broken');
  await expect(broken.locator('.diagram-error')).toHaveText(/could not be drawn/, { timeout: 10_000 });
  await expect(broken).toContainText('shelf --> --> ((');
});

test('a block that an agent writes shows within 2 seconds of the write', async ({ page }) => {
  await page.goto(address());
  await expect(page.locator('#log-title')).toBeVisible();
  write([{ id: 'live', kind: 'note', stage: 'shape', title: 'Written while open', markdown: 'Fresh' }]);
  await expect(page.locator('#block-live')).toBeVisible({ timeout: LIVE });
});

test('a choice the person has made and not sent is kept through a change', async ({ page }) => {
  await page.goto(address());
  const bottom = page.locator('input[name="answer-q1"][value="Bottom"]');
  await bottom.check();
  write([{ id: 'meanwhile', kind: 'note', stage: 'shape', title: 'Meanwhile', markdown: 'x' }]);
  await expect(page.locator('#block-meanwhile')).toBeVisible({ timeout: LIVE });
  await expect(page.locator('input[name="answer-q1"][value="Bottom"]')).toBeChecked();
});

test('an answer given in Claude Code shows within 2 seconds, and the count of what waits changes with it', async ({ page }) => {
  await page.goto(address());
  await expect(page).toHaveTitle('(1) Yard');
  answerInTerminal([{ block: 'q1', labels: ['Top'] }]);
  const card = page.locator('#block-q1');
  await expect(card.locator('.answer-text')).toHaveText('Top', { timeout: LIVE });
  await expect(card.locator('.answer .byline')).toContainText('in the terminal');
  await expect(page).toHaveTitle('Yard', { timeout: LIVE });
  ask('q2', 'Which colour?');
  await expect(page).toHaveTitle('(1) Yard', { timeout: LIVE });
});

test('the page sends every answer of a submission at once, and shows them given on the page', async ({ page }) => {
  await page.goto(address());
  const panel = page.locator('.submission', { has: page.locator('legend', { hasText: 'Which colour?' }) });
  const send = panel.getByRole('button', { name: 'Send answers' });
  await expect(send).toBeDisabled();
  await panel.locator('input[value="Blue"]').check();
  await expect(send).toBeEnabled();
  const before = (await read()).revision;
  await send.click();
  const card = page.locator('#block-q2');
  await expect(card.locator('.answer-text')).toHaveText('Blue', { timeout: LIVE });
  await expect(card.locator('.answer .byline')).toContainText('on the page');
  expect((await read()).revision).toBe(before + 1);
});

test('the page shows the first answer with the channel it came from', async ({ page }) => {
  ask('q3', 'Which size?');
  await deaf(page);
  await page.goto(address());
  const panel = page.locator('.submission', { has: page.locator('legend', { hasText: 'Which size?' }) });
  await expect(panel).toBeVisible();
  await panel.locator('input[value="Blue"]').check();
  answerInTerminal([{ block: 'q3', labels: ['Red'] }]);
  await panel.getByRole('button', { name: 'Send answers' }).click();
  await expect(page.locator('#notice')).toContainText('Answered elsewhere first');
  await expect(page.locator('#notice')).toContainText('q3 in the terminal');
  const card = page.locator('#block-q3');
  await expect(card.locator('.answer-text')).toHaveText('Red');
  await expect(card.locator('.answer .byline')).toContainText('in the terminal');
});

test('the page shows a block approved once the person approves it, and the count of what waits changes', async ({ page }) => {
  approval('ship', 'Ship it');
  await page.goto(address());
  await expect(page).toHaveTitle('(1) Yard');
  await page.locator('#approve-ship').click();
  await expect(page.locator('#block-ship .verdict-word')).toHaveText('Approved', { timeout: LIVE });
  await expect(page.locator('#block-ship .outcome')).toContainText('on the page');
  await expect(page).toHaveTitle('Yard', { timeout: LIVE });
});

test('the count of what waits changes within 2 seconds of a submission being withdrawn', async ({ page }) => {
  const key = approval('varnish', 'Varnish it');
  await page.goto(address());
  await expect(page).toHaveTitle('(1) Yard');
  withdraw(key);
  await expect(page).toHaveTitle('Yard', { timeout: LIVE });
});

test('a choice the person made and did not send is kept through a reload of the page', async ({ page }) => {
  ask('q5', 'Which wood?');
  await page.goto(address());
  await page.locator('input[name="answer-q5"][value="Blue"]').check();
  await page.reload();
  await expect(page.locator('input[name="answer-q5"][value="Blue"]')).toBeChecked();
  answerInTerminal([{ block: 'q5', labels: ['Blue'] }]);
});

test('a rejection without a reason records nothing and asks for the reason; with one, the page shows it rejected', async ({ page }) => {
  approval('paint', 'Paint it');
  await page.goto(address());
  const before = (await read()).revision;
  await page.locator('#reject-paint').click();
  await expect(page.locator('.decision-form').filter({ has: page.locator('#reject-paint') }).locator('.send-message')).toContainText('Give the reason');
  expect((await read()).revision).toBe(before);
  await page.locator('#reason-paint').fill('Not this colour');
  write([{ id: 'typing', kind: 'note', stage: 'shape', title: 'Written while the person types', markdown: 'x' }]);
  await expect(page.locator('#block-typing')).toBeVisible({ timeout: LIVE });
  await expect(page.locator('#reason-paint')).toHaveValue('Not this colour');
  await page.locator('#reject-paint').click();
  await expect(page.locator('#block-paint .verdict-word')).toHaveText('Rejected', { timeout: LIVE });
  await expect(page.locator('#block-paint .outcome')).toContainText('Not this colour');
});

test('the page shows the first outcome with the channel it came from', async ({ page }) => {
  approval('sand', 'Sand it');
  await deaf(page);
  await page.goto(address());
  await expect(page.locator('#reason-sand')).toBeVisible();
  answerInTerminal([{ block: 'sand', verdict: 'approved' }]);
  await page.locator('#reason-sand').fill('Too rough');
  await page.locator('#reject-sand').click();
  await expect(page.locator('#notice')).toContainText('decided elsewhere first');
  await expect(page.locator('#block-sand .verdict-word')).toHaveText('Approved');
  await expect(page.locator('#block-sand .outcome')).toContainText('in the terminal');
});

test('a Yard Log opened in another tree the page server serves shows in the list within 2 seconds', async ({ page }) => {
  const registered = await postAsMod('/api/register', { tree: setup.other.cwd });
  expect(registered.status).toBe(200);
  await page.goto(address());
  await expect(page.locator('.trees > .repository')).toHaveCount(2, { timeout: LIVE });
  // The other tree is its repository's main worktree, named once, by the repository's heading.
  const other = page.locator(`.repository:has(> h2[title="${setup.other.cwd}"]) .log-item`);
  await expect(other).toHaveCount(0);
  writeOther([{ id: 'docs', kind: 'stage', title: 'Write the docs', status: 'active' }]);
  await expect(other).toHaveCount(1, { timeout: LIVE });
});

test('the revision of the last visit is kept in the browser, and what changed since is marked and counted', async ({ page }) => {
  await page.goto(address());
  await expect(page.locator('#log-title')).toBeVisible();
  await expect(page.locator('.changed-count')).toHaveCount(0);
  const key = (await read()).yardLog.key;
  const kept = await page.evaluate(() => JSON.parse(localStorage.getItem('yard.visits') ?? '{}'));
  expect(kept[key]).toBe((await read()).revision);
  await page.goto('about:blank');
  write([{ id: 'away', kind: 'note', stage: 'shape', title: 'Written while away', markdown: 'x' }]);
  await page.goto(address());
  await expect(page.locator('.changed-count')).toHaveText('1 block changed since your last visit');
  await expect(page.locator('#block-away')).toHaveClass(/is-new/);
});

test('the Gantt chart is chosen beside the order, kept for the next visit, follows changes and fills the window', async ({ page }) => {
  await page.goto(address());
  await page.getByRole('button', { name: 'Gantt chart' }).click();
  const chart = page.locator('svg.gantt-chart');
  await expect(chart).toBeVisible();
  await page.reload();
  await expect(chart).toBeVisible();
  write([
    { id: 'shape', kind: 'stage', title: 'Shape the shelf', status: 'done' },
    { id: 'build', kind: 'stage', title: 'Build the shelf', status: 'active' },
  ]);
  await expect(chart.locator('.stage-label', { hasText: 'Build the shelf' })).toBeVisible({ timeout: LIVE });
  const wide = Number(await chart.getAttribute('width'));
  expect(wide).toBeGreaterThan(1280 - 300 - 200);
  await page.setViewportSize({ width: 1600, height: 900 });
  await expect.poll(async () => Number(await chart.getAttribute('width')), { timeout: LIVE }).toBeGreaterThan(wide + 200);
});

test('choosing a stage in the Gantt chart opens the timeline at it', async ({ page }) => {
  await page.goto(address());
  await page.getByRole('button', { name: 'Gantt chart' }).click();
  await page.locator('.stage-bar', { hasText: 'Shape the shelf' }).click();
  await expect(page.locator('ol.timeline')).toBeVisible();
  expect(new URLSearchParams(new URL(page.url()).hash.slice(1)).get('block')).toBe('shape');
  await expect(page.locator('#block-shape')).toHaveAttribute('open', '');
  await expect(page.locator('#block-shape')).toHaveClass(/is-picked/);
});

test('an open page shows a counted request’s tokens within 2 seconds of the request’s end', async ({ page }) => {
  await page.goto(address());
  await expect(page.locator('#tokens')).toBeVisible();
  const events = [
    { session: 's1', dir: setup.tree, at: since(-30), kind: 'turn.start' },
    {
      session: 's1',
      dir: setup.tree,
      at: since(-1),
      kind: 'request',
      model: 'claude-opus-5-5',
      tokens: { input: 4321, cacheRead: 0, cacheWrite: 0, output: 99 },
    },
  ];
  const posted = await postAsMod('/api/events', events);
  expect(posted.status).toBe(200);
  await expect(page.locator('.token-totals dd').first()).toHaveAttribute('title', '4321', { timeout: LIVE });
});
