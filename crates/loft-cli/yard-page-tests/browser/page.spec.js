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

/** The computed colour of `property` on the first element `locator` finds. */
function colour(locator, property) {
  return locator.first().evaluate((element, name) => getComputedStyle(element).getPropertyValue(name), property);
}

/** A box of the diagram drawn in the block `drawn`. */
function diagramBox(page) {
  return page.locator('#block-drawn svg g.node').first().locator('rect, path, polygon');
}

test('in a browser that asks for a dark scheme, the page, its Gantt chart and its diagrams are drawn in their dark colours', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.goto(address());
  expect(await colour(page.locator('body'), 'background-color')).toBe('rgb(12, 17, 23)');
  await expect(diagramBox(page).first()).toBeVisible({ timeout: 10_000 });
  expect(await colour(diagramBox(page), 'fill')).toBe('rgb(24, 33, 43)');
  await page.getByRole('button', { name: 'Gantt chart' }).click();
  expect(await colour(page.locator('.gantt .lane-label'), 'color')).toBe('rgb(232, 238, 244)');
});

test('in a browser that asks for a light scheme, the page, its Gantt chart and its diagrams are drawn in their light colours', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light' });
  await page.goto(address());
  expect(await colour(page.locator('body'), 'background-color')).toBe('rgb(255, 255, 255)');
  await expect(diagramBox(page).first()).toBeVisible({ timeout: 10_000 });
  expect(await colour(diagramBox(page), 'fill')).toBe('rgb(243, 247, 250)');
  await page.getByRole('button', { name: 'Gantt chart' }).click();
  expect(await colour(page.locator('.gantt .lane-label'), 'color')).toBe('rgb(14, 20, 29)');
});

test('when the browser changes its scheme, the open page and each diagram it drew turn to it without a reload', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light' });
  await page.goto(address());
  await expect(diagramBox(page).first()).toBeVisible({ timeout: 10_000 });
  await page.evaluate(() => {
    /** @type {any} */ (window).unreloaded = true;
  });
  await page.emulateMedia({ colorScheme: 'dark' });
  await expect.poll(() => colour(page.locator('body'), 'background-color')).toBe('rgb(12, 17, 23)');
  await expect.poll(() => colour(diagramBox(page), 'fill'), { timeout: 10_000 }).toBe('rgb(24, 33, 43)');
  expect(await page.evaluate(() => /** @type {any} */ (window).unreloaded)).toBe(true);
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

test('the Gantt chart is chosen, kept for the next visit, follows changes, and fits the page as the window narrows', async ({ page }) => {
  await page.goto(address());
  await page.getByRole('button', { name: 'Gantt chart' }).click();
  const chart = page.locator('.gantt');
  await expect(chart).toBeVisible();
  await page.reload();
  await expect(chart).toBeVisible();
  write([
    { id: 'shape', kind: 'stage', title: 'Shape the shelf', status: 'done' },
    { id: 'build', kind: 'stage', title: 'Build the shelf', status: 'active' },
  ]);
  await expect(chart.locator('.stage-name', { hasText: 'Build the shelf' })).toBeVisible({ timeout: LIVE });
  const width = async (locator) => (await locator.boundingBox())?.width ?? 0;
  const main = page.locator('#log');
  expect(await width(chart)).toBeLessThanOrEqual((await width(main)) + 1);
  await page.setViewportSize({ width: 1000, height: 900 });
  await expect.poll(async () => (await width(chart)) - (await width(main)), { timeout: LIVE }).toBeLessThanOrEqual(1);
  await page.setViewportSize({ width: 1280, height: 900 });
});

test('the page keeps its width and its switch of views stays in place whichever view is shown', async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 900 });
  await page.goto(address());
  const main = page.locator('#log');
  const timeline = page.getByRole('button', { name: 'Timeline' });
  await timeline.click();
  await expect(page.locator('ol.timeline')).toBeVisible();
  const before = { main: await main.boundingBox(), switch: await timeline.boundingBox() };
  await page.getByRole('button', { name: 'Gantt chart' }).click();
  await expect(page.locator('.gantt')).toBeVisible();
  const after = { main: await main.boundingBox(), switch: await timeline.boundingBox() };
  expect(after.main?.width).toBe(before.main?.width);
  expect(after.switch?.x).toBe(before.switch?.x);
  expect(after.switch?.y).toBe(before.switch?.y);
  await page.getByRole('button', { name: 'Timeline' }).click();
  await page.setViewportSize({ width: 1280, height: 900 });
});

/**
 * A long day of work, as the page server would give it: twelve stages of fifteen minutes with
 * three tasks each, the last stage's title far too long for the column, in two turns with a
 * folded idle stretch of ten minutes between them.
 */
function longDay() {
  const start = Date.now() - 4 * 3600_000;
  const iso = (minutes) => new Date(start + minutes * 60_000).toISOString().replace(/\.\d+Z$/, 'Z');
  const by = { host: 'claude-code', session: 's1' };
  let keys = 0;
  const key = () => `00000000-0000-7000-9000-${String((keys += 1)).padStart(12, '0')}`;
  const version = (body, minutes) => ({ key: key(), number: 1, act: { at: iso(minutes), revision: 1, by }, body });
  const stages = Array.from({ length: 12 }, (_, index) => {
    const from = index * 15 + (index >= 6 ? 10 : 0);
    const title = index === 11 ? 'The last stage, whose title runs on far longer than any column of names could ever hold' : `Stage ${index + 1}`;
    const tasks = [0, 1, 2].map((each) => ({
      key: key(),
      id: `long-${index}-task-${each}`,
      kind: 'task',
      version: version({ title: `Task ${each + 1} of stage ${index + 1}`, state: 'done' }, from + 5 * each + 5),
      times: { listed: iso(from), started: iso(from + 5 * each), ended: iso(from + 5 * each + 5) },
    }));
    return {
      key: key(),
      id: `long-${index}`,
      kind: 'stage',
      owner: by,
      times: { listed: iso(from), started: iso(from), ended: iso(from + 15) },
      version: version({ title, status: 'done', goal: title }, from + 15),
      blocks: tasks,
    };
  });
  const turns = [
    { started: iso(0), ended: iso(90) },
    { started: iso(100), ended: iso(190) },
  ];
  return { stages, sent: [], activity: [{ session: 's1', turns, plan: [], spec: [], loft: [] }] };
}

/** Opens the page on the long day, in the Gantt chart, deaf to changes; gives the chart's frame. */
async function openLongDay(page) {
  await deaf(page);
  await page.route('**/api/log?*', async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, json: { ...(await response.json()), ...longDay() } });
  });
  await page.goto(address());
  await page.getByRole('button', { name: 'Gantt chart' }).click();
  const frame = page.locator('.gantt-frame');
  await expect(frame).toBeVisible();
  return frame;
}

test('a chart whose time needs more room than the page scrolls sideways within its frame, opening at its latest time, and the page does not', async ({ page }) => {
  const frame = await openLongDay(page);
  const sizes = await frame.evaluate((element) => ({ left: element.scrollLeft, width: element.scrollWidth, shown: element.clientWidth }));
  expect(sizes.width).toBeGreaterThan(sizes.shown);
  expect(sizes.left + sizes.shown).toBeGreaterThanOrEqual(sizes.width - 1);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  const fifteen = await page.locator('.stage-bar').first().boundingBox();
  expect(fifteen?.width ?? 0).toBeGreaterThanOrEqual(15 * 8 - 1);
});

test('while the chart scrolls sideways its names stay in view, and while it scrolls down its axis does, in a frame no taller than the window', async ({ page }) => {
  const frame = await openLongDay(page);
  const name = page.locator('.stage-name').first();
  await frame.evaluate((element) => (element.scrollLeft = 0));
  const before = await name.boundingBox();
  await frame.evaluate((element) => (element.scrollLeft = 400));
  expect((await name.boundingBox())?.x).toBe(before?.x);
  const head = page.locator('.gantt-head');
  const top = await head.boundingBox();
  await frame.evaluate((element) => (element.scrollTop = 300));
  expect(await frame.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
  expect((await head.boundingBox())?.y).toBe(top?.y);
  expect((await frame.boundingBox())?.height ?? 0).toBeLessThanOrEqual(900);
});

test('every name stays inside the column, a long one cut short with an ellipsis', async ({ page }) => {
  await openLongDay(page);
  const crossing = await page.evaluate(() =>
    [...document.querySelectorAll('.name-cell')].filter((cell) => {
      const name = /** @type {HTMLElement | null} */ (cell.querySelector('.lane-label, .stage-name, .task-name'));
      return name && name.getBoundingClientRect().right > cell.getBoundingClientRect().right + 0.5;
    }).length,
  );
  expect(crossing).toBe(0);
  const long = page.locator('.stage-name', { hasText: 'The last stage' });
  expect(await long.evaluate((element) => getComputedStyle(element).textOverflow)).toBe('ellipsis');
  expect(await long.evaluate((element) => element.scrollWidth > element.clientWidth)).toBe(true);
});

test('dragging the column’s edge widens it within its bounds, and the next visit keeps the width', async ({ page }) => {
  await openLongDay(page);
  const cell = page.locator('.stage-row .name-cell').first();
  const width = async () => (await cell.boundingBox())?.width ?? 0;
  const first = await width();
  const edge = await page.locator('.names-edge').boundingBox();
  if (!edge) throw new Error('the column has no edge');
  const x = edge.x + edge.width / 2;
  const y = edge.y + edge.height / 2;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + 60, y, { steps: 4 });
  await page.mouse.up();
  await expect.poll(width).toBeGreaterThan(first + 55);
  const dragged = await width();
  const again = await page.locator('.names-edge').boundingBox();
  if (!again) throw new Error('the column has no edge');
  await page.mouse.move(again.x + again.width / 2, y);
  await page.mouse.down();
  await page.mouse.move(again.x + 1000, y, { steps: 4 });
  await page.mouse.up();
  await expect.poll(width).toBeLessThanOrEqual(481);
  await page.reload();
  await page.locator('.gantt-frame').waitFor();
  expect(await width()).toBeGreaterThan(dragged);
  await page.evaluate(() => localStorage.removeItem('yard.gantt-names'));
});

test('a faint grid lies behind the bars, and a folded idle stretch is unhatched', async ({ page }) => {
  await openLongDay(page);
  expect(await page.locator('.stage-row').first().evaluate((element) => getComputedStyle(element).borderTopWidth)).toBe('1px');
  const lines = page.locator('.gantt-lane .grid-line');
  expect(await lines.count()).toBe(await page.locator('.gantt-axis .tick').count());
  expect(await lines.first().evaluate((element) => getComputedStyle(element).borderLeftWidth)).toBe('1px');
  expect(await page.locator('.idle-gap').first().evaluate((element) => getComputedStyle(element).backgroundImage)).toBe('none');
});

test('choosing a stage in the Gantt chart opens the timeline at it', async ({ page }) => {
  await page.goto(address());
  await page.getByRole('button', { name: 'Gantt chart' }).click();
  await page.locator('.stage-name', { hasText: 'Shape the shelf' }).click();
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
