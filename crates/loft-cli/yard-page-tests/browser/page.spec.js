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

/** The modal of what waits on the person. */
function waitingModal(page) {
  return page.locator('dialog.modal[data-modal="waiting"]');
}

/** Closes the modal of what waits, as the person does to work the page behind it. */
async function closeWaiting(page) {
  await waitingModal(page).getByRole('button', { name: 'Close' }).click();
  await expect(waitingModal(page)).toBeHidden();
}

/** Keeps the page from hearing of changes, so that it shows what it loaded until it loads again. */
async function deaf(page) {
  await page.route('**/api/stream', (route) => route.abort());
}

test('the person can fold and open each stage, the active one open and the planned one folded', async ({ page }) => {
  await page.goto(address());
  await closeWaiting(page);
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
  await closeWaiting(page);
  expect(await colour(page.locator('body'), 'background-color')).toBe('rgb(12, 17, 23)');
  await expect(diagramBox(page).first()).toBeVisible({ timeout: 10_000 });
  expect(await colour(diagramBox(page), 'fill')).toBe('rgb(24, 33, 43)');
  await page.getByRole('button', { name: 'Gantt chart' }).click();
  expect(await colour(page.locator('.gantt .lane-label'), 'color')).toBe('rgb(232, 238, 244)');
});

/** The mark's Y and the header's switch of views in `scheme`, whose ink and action blue are given. */
async function headerColours(page, scheme, ink, signal) {
  await page.emulateMedia({ colorScheme: scheme });
  await page.goto(address());
  await closeWaiting(page);
  expect(await colour(page.locator('.mark-initial'), 'color')).toBe(signal);
  expect(await colour(page.locator('.mark-word'), 'color')).toBe(ink);
  const shown = page.locator('header.top nav.views [aria-pressed="true"]');
  await expect(shown).toHaveText('Timeline');
  expect(await colour(shown, 'color')).toBe(ink);
  expect(await colour(shown, 'text-decoration-line')).toBe('underline');
  expect(await colour(shown, 'text-decoration-color')).toBe(signal);
  const other = page.locator('header.top nav.views [aria-pressed="false"]');
  expect(await colour(other, 'text-decoration-line')).toBe('none');
  expect(await colour(other, 'border-top-width')).toBe('0px');
}

test('in a light scheme, the mark’s Y is action blue and the rest ink, and the view shown in the header’s switch is ink underlined in action blue', async ({ page }) => {
  await headerColours(page, 'light', 'rgb(14, 20, 29)', 'rgb(20, 72, 143)');
});

test('in a dark scheme, the mark’s Y is action blue and the rest ink, and the view shown in the header’s switch is ink underlined in action blue', async ({ page }) => {
  await headerColours(page, 'dark', 'rgb(232, 238, 244)', 'rgb(142, 197, 242)');
});

test('the mark’s Y meets the rest of its name with no gap between them', async ({ page }) => {
  await page.goto(address());
  const gap = await page.locator('.mark-initial').evaluate((initial) => {
    const rest = document.createRange();
    rest.selectNodeContents(/** @type {Node} */ (initial.nextSibling));
    return rest.getBoundingClientRect().left - initial.getBoundingClientRect().right;
  });
  expect(Math.abs(gap)).toBeLessThan(1);
});

test('in a browser that asks for a light scheme, the page, its Gantt chart and its diagrams are drawn in their light colours', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light' });
  await page.goto(address());
  await closeWaiting(page);
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

/** The modal of an expanded diagram. */
function diagramModal(page) {
  return page.locator('dialog.modal[data-modal="diagram"]');
}

/** Expands the diagram of the block `drawn`, and gives the stage it zooms and pans on. */
async function expandDiagram(page) {
  await page.goto(address());
  await closeWaiting(page);
  const drawn = page.locator('#block-drawn');
  await expect(drawn.locator('.diagram svg')).toBeVisible({ timeout: 10_000 });
  await drawn.getByRole('button', { name: 'Expand the diagram' }).click();
  const stage = diagramModal(page).locator('.zoom-stage');
  await expect(stage).toHaveClass(/is-fitted/);
  await expect(stage.locator('svg')).toBeVisible();
  return stage;
}

/** The box of the expanded diagram on the screen. */
async function diagramOnScreen(page) {
  return /** @type {{ x: number, y: number, width: number, height: number }} */ (await diagramModal(page).locator('.zoom-content svg').boundingBox());
}

/** The zoom the modal shows, as a number of percent. */
async function zoomShown(page) {
  return Number((await diagramModal(page).locator('.zoom-percent').textContent())?.replace('%', ''));
}

test('expanding a diagram opens it in the modal, named by its block, over the viewport within its margin and fitted whole', async ({ page }) => {
  const stage = await expandDiagram(page);
  const modal = diagramModal(page);
  await expect(modal.locator('.modal-head h2')).toHaveText('A diagram');
  const box = /** @type {{ x: number, y: number, width: number, height: number }} */ (await modal.boundingBox());
  const viewport = /** @type {{ width: number, height: number }} */ (page.viewportSize());
  expect(Math.round(box.x)).toBe(24);
  expect(Math.round(box.y)).toBe(24);
  expect(Math.round(viewport.width - box.x - box.width)).toBe(24);
  expect(Math.round(viewport.height - box.y - box.height)).toBe(24);
  await expect(stage).toBeFocused();
  await expect
    .poll(async () => {
      const room = /** @type {{ x: number, y: number, width: number, height: number }} */ (await stage.boundingBox());
      const diagram = await diagramOnScreen(page);
      const inside = diagram.x >= room.x - 0.5 && diagram.y >= room.y - 0.5;
      const within = diagram.x + diagram.width <= room.x + room.width + 0.5 && diagram.y + diagram.height <= room.y + room.height + 0.5;
      const centred = Math.abs(diagram.x - room.x - (room.x + room.width - diagram.x - diagram.width)) < 1;
      return inside && within && centred;
    })
    .toBe(true);
});

test('a pinch or the wheel with Ctrl held zooms about the pointer, the wheel and a drag pan, and neither the modal nor the page scrolls', async ({ page }) => {
  const stage = await expandDiagram(page);
  await expect.poll(() => zoomShown(page)).toBeGreaterThan(0);
  const before = await diagramOnScreen(page);
  const pointer = { x: before.x + before.width * 0.3, y: before.y + before.height * 0.6 };
  const scrolled = await page.evaluate(() => window.scrollY);
  const fitted = await zoomShown(page);
  await page.mouse.move(pointer.x, pointer.y);
  // A pinch on a trackpad reaches Chromium as the wheel with Ctrl held.
  await page.keyboard.down('Control');
  await page.mouse.wheel(0, -100);
  await page.keyboard.up('Control');
  await expect.poll(() => zoomShown(page)).toBeGreaterThan(fitted);
  const zoomed = await diagramOnScreen(page);
  expect(zoomed.width).toBeGreaterThan(before.width);
  expect((pointer.x - zoomed.x) / zoomed.width).toBeCloseTo((pointer.x - before.x) / before.width, 2);
  expect((pointer.y - zoomed.y) / zoomed.height).toBeCloseTo((pointer.y - before.y) / before.height, 2);

  await page.mouse.wheel(40, 60);
  await expect.poll(async () => (await diagramOnScreen(page)).x).toBeCloseTo(zoomed.x - 40, 0);
  const wheeled = await diagramOnScreen(page);
  expect(wheeled.y).toBeCloseTo(zoomed.y - 60, 0);
  expect(wheeled.width).toBeCloseTo(zoomed.width, 3);

  await page.mouse.move(pointer.x, pointer.y);
  await page.mouse.down();
  await page.mouse.move(pointer.x + 50, pointer.y + 30, { steps: 5 });
  await page.mouse.up();
  const dragged = await diagramOnScreen(page);
  expect(dragged.x).toBeCloseTo(wheeled.x + 50, 0);
  expect(dragged.y).toBeCloseTo(wheeled.y + 30, 0);

  expect(await page.evaluate(() => window.scrollY)).toBe(scrolled);
  expect(await diagramModal(page).locator('.modal-body').evaluate((body) => body.scrollTop)).toBe(0);
  await expect(stage).toBeVisible();
});

test('the keys and the buttons zoom, fit and pan the expanded diagram, and closing it gives the focus back to its button', async ({ page }) => {
  const stage = await expandDiagram(page);
  await expect.poll(() => zoomShown(page)).toBeGreaterThan(0);
  const fitted = await zoomShown(page);
  await stage.press('+');
  await expect.poll(() => zoomShown(page)).toBeGreaterThan(fitted);
  await stage.press('0');
  await expect.poll(() => zoomShown(page)).toBe(fitted);
  await stage.press('-');
  await expect.poll(() => zoomShown(page)).toBeLessThan(fitted);
  const before = await diagramOnScreen(page);
  await stage.press('ArrowLeft');
  await expect.poll(async () => (await diagramOnScreen(page)).x).toBeGreaterThan(before.x);
  await stage.press('ArrowUp');
  await expect.poll(async () => (await diagramOnScreen(page)).y).toBeGreaterThan(before.y);

  const modal = diagramModal(page);
  await modal.getByRole('button', { name: 'Fit the whole diagram' }).click();
  await expect.poll(() => zoomShown(page)).toBe(fitted);
  await modal.getByRole('button', { name: 'Zoom in' }).click();
  await expect.poll(() => zoomShown(page)).toBeGreaterThan(fitted);
  await modal.getByRole('button', { name: 'Zoom out' }).click();
  await modal.getByRole('button', { name: 'Zoom out' }).click();
  await expect.poll(() => zoomShown(page)).toBeLessThan(fitted);

  await stage.press('Escape');
  await expect(modal).toBeHidden();
  await expect(page.locator('#block-drawn').getByRole('button', { name: 'Expand the diagram' })).toBeFocused();
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

test('the open submissions wait in a modal over the darkened page, which takes no clicks, and nothing of them shows above the stages', async ({ page }) => {
  await page.goto(address());
  const modal = waitingModal(page);
  await expect(modal).toBeVisible();
  await expect(modal.locator('.submission legend', { hasText: 'Which shelf?' })).toBeVisible();
  await expect(page.locator('#log .submission')).toHaveCount(0);
  const scrim = await modal.evaluate((dialog) => getComputedStyle(dialog, '::backdrop').backgroundColor);
  expect(scrim).toMatch(/^rgba\(\d+, \d+, \d+, 0\.\d+\)$/);
  // A point in the margin, over the menu, and one at a stage's fold, behind the modal: neither
  // reaches the page.
  const shape = page.locator('#block-shape');
  const fold = await shape.locator(':scope > summary').boundingBox();
  for (const [x, y] of [[8, 300], [fold.x + 10, fold.y + 5]]) {
    expect(await page.evaluate(([x, y]) => Boolean(document.elementFromPoint(x, y)?.closest('dialog')), [x, y])).toBe(true);
  }
  await page.mouse.click(fold.x + 10, fold.y + 5);
  await expect(shape).toHaveAttribute('open', '');
});

test('the modal keeps its margin to each edge of the viewport and follows the viewport as it changes', async ({ page }) => {
  await page.goto(address());
  const modal = waitingModal(page);
  await expect(modal).toBeVisible();
  expect(await modal.boundingBox()).toEqual({ x: 24, y: 24, width: 1280 - 48, height: 900 - 48 });
  await page.setViewportSize({ width: 600, height: 700 });
  await expect.poll(() => modal.boundingBox()).toEqual({ x: 16, y: 16, width: 600 - 32, height: 700 - 32 });
  await page.setViewportSize({ width: 1280, height: 900 });
});

test('the person closes the modal by its button or Escape and it stays closed through a change, until they open it again or a new submission is sent', async ({ page }) => {
  await page.goto(address());
  const modal = waitingModal(page);
  await expect(modal).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(modal).toBeHidden();
  write([{ id: 'while-closed', kind: 'note', stage: 'shape', title: 'Written while it is closed', markdown: 'x' }]);
  await expect(page.locator('#block-while-closed')).toBeVisible({ timeout: LIVE });
  await expect(modal).toBeHidden();
  const button = page.locator('.top #open-waiting');
  await expect(button).toHaveText('1 waits on you');
  await button.click();
  await expect(modal).toBeVisible();
  await closeWaiting(page);
  await expect(button).toBeFocused();
  await page.locator('#block-q1').getByRole('button', { name: 'Answer it' }).click();
  await expect(modal).toBeVisible();
  await expect(modal.locator(`input[name="answer-q1"]`).first()).toBeFocused();
  await closeWaiting(page);
  ask('q-again', 'Which again?');
  await expect(modal).toBeVisible({ timeout: LIVE });
  await expect(modal.locator('.submission')).toHaveCount(2);
  answerInTerminal([{ block: 'q-again', labels: ['Red'] }]);
  await expect(modal.locator('.submission')).toHaveCount(1, { timeout: LIVE });
});

test('the button that opens what waits lies in the page’s header after the switch of views, and stays in view while the Yard Log scrolls', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 400 });
  await page.goto(address());
  await closeWaiting(page);
  const button = page.locator('header.top #open-waiting');
  await expect(button).toHaveText('1 waits on you');
  expect((await box(button)).x).toBeGreaterThan((await box(page.locator('header.top nav.views'))).x);
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  await expect.poll(() => page.evaluate(() => window.scrollY)).toBeGreaterThan(0);
  await expect(button).toBeInViewport();
  await button.click();
  await expect(waitingModal(page)).toBeVisible();
  await closeWaiting(page);
  await page.setViewportSize({ width: 1280, height: 900 });
});

test('content taller than the modal scrolls within it under its heading, and a change drawn into it keeps its scroll and focus', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 360 });
  await page.goto(address());
  const modal = waitingModal(page);
  await expect(modal).toBeVisible();
  const body = modal.locator('.modal-body');
  const head = modal.locator('.modal-head');
  expect(await body.evaluate((element) => element.scrollHeight > element.clientHeight)).toBe(true);
  const top = (await head.boundingBox()).y;
  await body.evaluate((element) => {
    element.scrollTop = 120;
  });
  expect((await head.boundingBox()).y).toBe(top);
  expect(await page.evaluate(() => window.scrollY)).toBe(0);
  const bottom = modal.locator('input[name="answer-q1"][value="Bottom"]');
  await bottom.focus();
  const scrolled = await body.evaluate((element) => element.scrollTop);
  ask('q-tall', 'Which tall?');
  await expect(modal.locator('.submission')).toHaveCount(2, { timeout: LIVE });
  await expect(bottom).toBeFocused();
  expect(await body.evaluate((element) => element.scrollTop)).toBe(scrolled);
  answerInTerminal([{ block: 'q-tall', labels: ['Red'] }]);
  await expect(modal.locator('.submission')).toHaveCount(1, { timeout: LIVE });
  await page.setViewportSize({ width: 1280, height: 900 });
});

test('a fold the person toggled stays so through a change and after a reload, and a stage takes its default once its status changes', async ({ page }) => {
  write([
    { id: 'kept-fold', kind: 'note', stage: 'shape', title: 'Folded at first', markdown: 'Held open.', folded: true },
    { id: 'kept-stage', kind: 'stage', title: 'A stage for later', status: 'planned' },
  ]);
  await page.goto(address());
  await closeWaiting(page);
  const note = page.locator('#block-kept-fold details');
  const shape = page.locator('#block-shape');
  const later = page.locator('#block-kept-stage');
  await note.locator('summary').click();
  await shape.locator(':scope > summary').click();
  await later.locator(':scope > summary').click();
  const held = async () => {
    await expect(note).toHaveAttribute('open', '');
    await expect(shape).not.toHaveAttribute('open', '');
    await expect(later).toHaveAttribute('open', '');
  };
  await held();
  write([{ id: 'kept-meanwhile', kind: 'stage', title: 'Written meanwhile', status: 'planned' }]);
  await expect(page.locator('#block-kept-meanwhile')).toBeVisible({ timeout: LIVE });
  await held();
  await page.reload();
  await expect(page.locator('#log-title')).toBeVisible();
  await closeWaiting(page);
  await held();
  write([{ id: 'kept-stage', kind: 'stage', title: 'A stage for later', status: 'done' }]);
  await expect(later.locator('.status')).toHaveText('Done', { timeout: LIVE });
  await expect(later).not.toHaveAttribute('open', '');
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
  await expect(waitingModal(page)).toBeHidden({ timeout: LIVE });
  await expect(page.locator('#notice')).toHaveText('Sent.');
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

test('the page begins at the window’s left and grows with the window up to 1800 pixels, any room beyond it at its right', async ({ page }) => {
  const layout = page.locator('.layout');
  await page.goto(address());
  for (const [window, wide] of [
    [1280, 1280],
    [2200, 1800],
  ]) {
    await page.setViewportSize({ width: window, height: 900 });
    await expect.poll(async () => (await layout.boundingBox())?.width).toBe(wide);
    expect((await layout.boundingBox())?.x).toBe(0);
  }
  await page.setViewportSize({ width: 1280, height: 900 });
});

test('the page keeps its width and its switch of views stays in place whichever view is shown', async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 900 });
  await page.goto(address());
  const main = page.locator('#log');
  const timeline = page.getByRole('button', { name: 'Timeline' });
  await timeline.click();
  await expect(page.locator('ol.timeline')).toBeVisible();
  await expect(page.locator('header.top nav.views').getByRole('button', { name: 'Timeline' })).toBeVisible();
  expect((await box(timeline)).x).toBeGreaterThan((await box(page.locator('header.top .mark'))).x);
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

/** The box of what `locator` finds, which must be drawn. */
async function box(locator) {
  const found = await locator.boundingBox();
  if (!found) throw new Error('nothing drawn');
  return found;
}

test('the inherited Yard Logs are folded beneath their count under the repository, listed when opened, filtered, and kept open', async ({ page }) => {
  await page.goto(address());
  const fold = page.locator('#side details.inherited');
  await expect(fold.locator('summary')).toHaveText('Inherited (2)');
  await expect(fold).not.toHaveAttribute('open', '');
  await expect(page.locator('#side .log-item.is-inherited')).toHaveCount(0);
  await fold.locator('summary').click();
  const items = fold.locator('.log-item.is-inherited');
  await expect(items).toHaveCount(2);
  await fold.locator('.inherited-filter').fill('LAMP');
  await expect(items).toHaveCount(1);
  await expect(items.locator('.branch')).toHaveText('old/lamp');
  await page.reload();
  await expect(fold).toHaveAttribute('open', '');
  await expect(items).toHaveCount(2);
  await items.locator('a', { hasText: 'old/desk' }).click();
  await expect(page.locator('#log-title')).toHaveText('old/desk');
  await page.evaluate(() => localStorage.removeItem('yard.inherited'));
});

test('the menu runs down the window’s left from top to bottom, beside the page’s header, and stays while the Yard Log scrolls', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 400 });
  await page.goto(address());
  const side = page.locator('#side');
  await expect(side.getByRole('heading', { name: 'Yard Logs' })).toBeVisible();
  const menu = await box(side);
  expect([menu.x, menu.y, menu.height]).toEqual([0, 0, 400]);
  expect((await box(page.locator('.top'))).x).toBe(menu.x + menu.width);
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  await expect.poll(() => page.evaluate(() => window.scrollY)).toBeGreaterThan(0);
  expect(Math.abs((await box(side)).y)).toBeLessThan(1);
  await page.setViewportSize({ width: 1280, height: 900 });
});

test('folding the menu leaves a strip and gives its room to the page, which begins as near the strip as it lay to the menu, and the next visit keeps it folded', async ({ page }) => {
  await page.goto(address());
  const side = page.locator('#side');
  const main = page.locator('#log');
  const header = page.locator('.top');
  const open = await box(side);
  const gap = (await box(main)).x - (open.x + open.width);
  const fold = page.getByRole('button', { name: 'Fold the menu of Yard Logs' });
  await expect(fold).toHaveAttribute('aria-expanded', 'true');
  await fold.click();
  await expect(page.locator('#menu-lists')).toBeHidden();
  await expect(side.getByRole('heading', { name: 'Yard Logs' })).toHaveCount(0);
  await expect.poll(async () => (await box(side)).width).toBe(58);
  const strip = await box(side);
  expect(strip.height).toBe(900);
  expect((await box(header)).x).toBe(strip.x + strip.width);
  expect((await box(main)).x).toBe(strip.x + strip.width + gap);
  await page.reload();
  const opener = page.getByRole('button', { name: /^Open the menu of Yard Logs/ });
  await expect(opener).toHaveAttribute('aria-expanded', 'false');
  expect((await box(side)).width).toBe(58);
  await opener.click();
  await expect(page.locator('#menu-lists')).toBeVisible();
  await expect.poll(async () => (await box(main)).x).toBe(open.x + open.width + gap);
  await page.evaluate(() => localStorage.removeItem('yard.list'));
});

test('in a window narrower than 860 pixels the open menu lies over the page, which keeps only the strip’s room', async ({ page }) => {
  await page.setViewportSize({ width: 700, height: 900 });
  await page.goto(address());
  const side = page.locator('#side');
  const menu = await box(side);
  const header = await box(page.locator('.top'));
  expect(menu.x).toBe(0);
  expect(header.x).toBe(58);
  expect(menu.x + menu.width).toBeGreaterThan(header.x + 200);
  await page.getByRole('button', { name: 'Fold the menu of Yard Logs' }).click();
  await expect.poll(async () => (await box(side)).width).toBe(58);
  expect((await box(page.locator('.top'))).x).toBe(58);
  await page.evaluate(() => localStorage.removeItem('yard.list'));
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

test('a chart the person scrolled opens again where they left it, after the page draws it again and after a reload', async ({ page }) => {
  const frame = await openLongDay(page);
  const left = () => frame.evaluate((element) => element.scrollLeft);
  expect(await left()).toBeGreaterThan(100);
  await frame.evaluate((element) => element.scrollTo({ left: 40 }));
  await expect.poll(left).toBe(40);
  await page.getByRole('button', { name: 'Timeline' }).click();
  await page.getByRole('button', { name: 'Gantt chart' }).click();
  await expect.poll(left).toBe(40);
  await page.reload();
  await frame.waitFor();
  await expect.poll(left).toBe(40);
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

test('the chart’s frame does not rebound at its edges, and a scroll down it can take no further scrolls the page', async ({ page }) => {
  const frame = await openLongDay(page);
  const behaviour = await frame.evaluate((element) => {
    const style = getComputedStyle(element);
    return [style.overscrollBehaviorX, style.overscrollBehaviorY];
  });
  expect(behaviour).toEqual(['none', 'auto']);
  await frame.evaluate((element) => (element.scrollTop = element.scrollHeight));
  await page.evaluate(() => window.scrollTo(0, 0));
  const box = await frame.boundingBox();
  if (!box) throw new Error('the chart has no frame');
  await page.mouse.move(box.x + box.width / 2, box.y + Math.min(box.height, 900 - box.y) / 2);
  await page.mouse.wheel(0, 400);
  await expect.poll(() => page.evaluate(() => window.scrollY)).toBeGreaterThan(0);
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

test('folding the menu widens the Gantt chart into the room the menu left, and opening it narrows the chart again', async ({ page }) => {
  const frame = await openLongDay(page);
  const width = () => frame.evaluate((element) => element.clientWidth);
  const before = await width();
  const menu = (await page.locator('#side').boundingBox())?.width ?? 0;
  await page.getByRole('button', { name: 'Fold the menu of Yard Logs' }).click();
  await expect.poll(width).toBe(before + menu - 58);
  await page.getByRole('button', { name: /^Open the menu of Yard Logs/ }).click();
  await expect.poll(width).toBe(before);
  await page.evaluate(() => localStorage.removeItem('yard.list'));
});

test('a faint grid lies behind the bars, its lines unbroken by folded idle stretches, which are unhatched', async ({ page }) => {
  await openLongDay(page);
  expect(await page.locator('.stage-row').first().evaluate((element) => getComputedStyle(element).borderTopWidth)).toBe('1px');
  const alpha = (colour) => {
    const found = /\/\s*([\d.]+)\)$/.exec(colour) ?? /rgba\([^)]*,\s*([\d.]+)\)$/.exec(colour);
    return found ? Number(found[1]) : colour === 'transparent' ? 0 : 1;
  };
  const gaps = await page.locator('.idle-gap').evaluateAll((elements) => elements.map((element) => getComputedStyle(element).backgroundColor));
  expect(gaps.length).toBeGreaterThan(0);
  for (const colour of gaps) expect(alpha(colour), `a gap of ${colour} lets the line beneath it through`).toBeLessThan(0.2);
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

test('the page draws the menu again only when the list, the Yard Log chosen or the menu’s fold changes', async ({ page }) => {
  await page.goto(address());
  const totals = page.locator('.token-totals dd').first();
  await expect(page.locator('#tokens')).toBeVisible();
  const before = await totals.getAttribute('title');
  // A mark on the menu as drawn, which a menu drawn again does not carry.
  await page.evaluate(() => document.querySelector('#side .menu-head')?.setAttribute('data-mark', 'drawn'));
  const marked = page.locator('#side .menu-head[data-mark="drawn"]');
  const request = { session: 's1', dir: setup.tree, at: since(-1), kind: 'request', model: 'claude-opus-5-5', tokens: { input: 1111, cacheRead: 0, cacheWrite: 0, output: 1 } };
  expect((await postAsMod('/api/events', [request])).status).toBe(200);
  await expect.poll(() => totals.getAttribute('title'), { timeout: LIVE }).not.toBe(before);
  await expect(marked).toHaveCount(1);
  await page.getByRole('button', { name: 'Fold the menu of Yard Logs' }).click();
  await expect(marked).toHaveCount(0);
  await page.evaluate(() => localStorage.removeItem('yard.list'));
});
