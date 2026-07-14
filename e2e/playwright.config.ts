import { defineConfig, devices } from '@playwright/test';

// Points at a manually-started weir server (see e2e/README). No webServer here —
// the server needs a specific env (WEIR_DEST_MANIFESTS_DIR + staged rest-dest).
export default defineConfig({
  testDir: './tests',
  // Serialize: the e2e server is single sqlite; parallel writers hit "database is locked"
  // (production is Postgres, where this isn't an issue). The suite is small, so serial is cheap.
  workers: 1,
  fullyParallel: false,
  timeout: 30_000,
  expect: { timeout: 10_000 },
  use: {
    baseURL: process.env.WEIR_UI_URL ?? 'http://localhost:8787',
    headless: true,
    trace: 'retain-on-failure',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
});
