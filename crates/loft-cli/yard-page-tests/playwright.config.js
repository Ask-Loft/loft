// The browser suite runs against the page server that `tests/yard_page_browser.rs` starts, which
// names it in the file `YARD_BROWSER_SETUP` names. Its tests share that server and its Yard Log,
// so they run one at a time, in order.

import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: 'browser',
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 30_000,
  reporter: [['list']],
  use: {
    browserName: 'chromium',
    headless: true,
    viewport: { width: 1280, height: 900 },
  },
});
